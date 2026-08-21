//! Wires the hook server to the app. Implements `HookSink` from peekle-server.
//!
//! Every field lookup here is optional. Claude Code adds fields between
//! versions, and the exact task payloads are pinned by captured fixtures
//! rather than by documentation. tech.md R-4.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::labels::classify;
use peekle_core::types::{
    PromptAnswer, PromptKind, PromptOutcome, PromptRequest, SessionStatus, TaskItem, TaskStatus,
    ToastRequest, ToastTone,
};
use peekle_core::FeedEvent;
use peekle_server::HookSink;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::events;
use crate::state::AppState;
use crate::windows;

pub struct AppSink {
    app: AppHandle,
    state: Arc<AppState>,
}

impl AppSink {
    pub fn new(app: AppHandle, state: Arc<AppState>) -> Self {
        Self { app, state }
    }

    fn emit_sessions(&self, cards: Vec<peekle_core::types::SessionCard>) {
        tracing::debug!(
            sessions = cards.len(),
            entries = cards.iter().map(|c| c.entries.len()).sum::<usize>(),
            "feed updated"
        );
        if let Err(err) = self.app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
    }
}

impl HookSink for AppSink {
    fn is_enabled(&self) -> bool {
        self.state.enabled()
    }

    fn open_prompt(&self, request: PromptRequest) -> oneshot::Receiver<PromptOutcome> {
        // Register before any window work: the channel has to exist before
        // anything can resolve it.
        let receiver = self.state.pending.register(request.id.clone());

        // Something typed while the agent was busy. This is the turn boundary
        // it was waiting for, so it leaves now and the island stays down: the
        // user already said what they wanted. tech.md 6.5.
        if request.kind == PromptKind::Stop {
            if let Some(text) = self.state.take_queued(&request.session.session_id) {
                let at = now_ms();
                self.state.end_turn(&request.session.session_id, at);
                let cards = self
                    .state
                    .replies_delivered(&request.session.session_id, at);
                self.emit_sessions(cards);

                self.state.pending.resolve(
                    &request.id,
                    PromptOutcome::Answered(PromptAnswer {
                        prompt_id: request.id.clone(),
                        choice: None,
                        text: Some(text),
                    }),
                );
                tracing::debug!(session = %request.session.session_id, "a queued reply left on this stop");
                return receiver;
            }
        }

        // A Stop means the turn is over, so nothing can still be in flight.
        // Whatever is still Running never reported success: PostToolUse does
        // not fire for a failed call. tech.md 6.3.
        if request.kind == PromptKind::Stop {
            let at = now_ms();
            self.state.end_turn(&request.session.session_id, at);
            self.state
                .set_session_status(&request.session, SessionStatus::WaitingOnUser, at);

            // What the agent said last belongs in the feed, not only in the
            // prompt: the user scrolls back to it. tech.md S6.
            if let Some(text) = request.last_message.as_deref() {
                self.state.assistant_turn(&request.session, text, at);
            }
            self.emit_sessions(self.state.sessions());
        }

        if self.state.claim_prompt(request.clone()) {
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                windows::open_prompt(&app, &request).await;
            });
        }
        receiver
    }

    fn prompt_timeout(&self) -> Duration {
        self.state.prompt_timeout()
    }

    /// One endpoint, three events. UserPromptSubmit, PreToolUse and PostToolUse
    /// all land here. tech.md 6.1.
    fn on_feed(&self, payload: &Value) {
        if let Some(event) = FeedEvent::from_payload(payload) {
            let cards = self.state.apply_feed(event, now_ms());
            self.emit_sessions(cards);
        }

        // A TodoWrite still carries the task list, which is a separate view of
        // the same turn. tech.md 6.3 and 6.6.
        let items = parse_tasks(payload);
        if items.is_empty() {
            return;
        }
        let merged = self.state.merge_tasks(items);
        if let Err(err) = self.app.emit(events::TASKS, &merged) {
            tracing::warn!(error = %err, "failed to emit tasks");
        }
    }

    /// SessionStart has never appeared in a capture, only SessionEnd, so a card
    /// is never created here. The registry opens one on the first feed event
    /// instead, which is the event that actually arrives. tech.md 6.1.
    fn on_session(&self, payload: &Value) {
        let Some(event) = payload.get("hook_event_name").and_then(Value::as_str) else {
            return;
        };
        let Some(session_id) = payload.get("session_id").and_then(Value::as_str) else {
            return;
        };

        match event {
            "SessionStart" => self.state.session_started(),
            "SessionEnd" => {
                self.state.session_ended();
                let at = now_ms();
                // A turn cannot outlive its session, so anything still running
                // never finished. tech.md 6.3.
                self.state.end_turn(session_id, at);
                let cards = self.state.mark_session_ended(session_id, at);
                self.emit_sessions(cards);
            }
            _ => {}
        }
    }

    fn on_notification(&self, payload: &Value) {
        let text = payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Claude needs you")
            .to_string();

        windows::toast(
            &self.app,
            ToastRequest {
                text,
                tone: ToastTone::Neutral,
                ttl_ms: 2600,
                badge: None,
            },
        );
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn status_of(raw: Option<&str>) -> TaskStatus {
    match raw {
        Some("in_progress") => TaskStatus::Active,
        Some("completed") => TaskStatus::Done,
        _ => TaskStatus::Pending,
    }
}

/// Reads a `TodoWrite` payload into task items. An unrecognised shape yields
/// an empty list rather than a partial one, so the feed never shows debris.
fn parse_tasks(payload: &Value) -> Vec<TaskItem> {
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let Some(todos) = payload
        .get("tool_input")
        .and_then(|input| input.get("todos"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    let now = now_ms();
    todos
        .iter()
        .enumerate()
        .filter_map(|(index, todo)| {
            let title = todo.get("content").and_then(Value::as_str)?.to_string();
            Some(TaskItem {
                id: format!("{session_id}:{index}"),
                label: classify(&title),
                title,
                status: status_of(todo.get("status").and_then(Value::as_str)),
                session_id: session_id.clone(),
                updated_at: now,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_todos_into_tasks() {
        let payload = json!({
            "session_id": "abc",
            "tool_input": {"todos": [
                {"content": "Fix the crash", "status": "in_progress"},
                {"content": "Ship it", "status": "pending"},
            ]}
        });

        let tasks = parse_tasks(&payload);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Fix the crash");
        assert_eq!(tasks[0].status, TaskStatus::Active);
        assert_eq!(tasks[0].label, peekle_core::types::TaskLabel::Bug);
        assert_eq!(tasks[1].status, TaskStatus::Pending);
    }

    #[test]
    fn an_unrecognised_payload_yields_nothing() {
        assert!(parse_tasks(&json!({})).is_empty());
        assert!(parse_tasks(&json!({"tool_input": {}})).is_empty());
        assert!(parse_tasks(&json!({"tool_input": {"todos": "nope"}})).is_empty());
    }

    #[test]
    fn a_todo_without_content_is_skipped_not_faked() {
        let payload = json!({"tool_input": {"todos": [{"status": "pending"}]}});
        assert!(parse_tasks(&payload).is_empty());
    }

    #[test]
    fn unknown_status_falls_back_to_pending() {
        assert_eq!(status_of(Some("something_new")), TaskStatus::Pending);
        assert_eq!(status_of(None), TaskStatus::Pending);
    }
}
