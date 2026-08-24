//! Wires the hook server to the app. Implements `HookSink` from peekle-server.
//!
//! Every field lookup here is optional. Claude Code adds fields between
//! versions, and the exact task payloads are pinned by captured fixtures
//! rather than by documentation. tech.md R-4.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::labels::classify;
use peekle_core::types::{
    PromptOutcome, PromptRequest, SessionStatus, TaskItem, TaskStatus, ToastRequest, ToastTone,
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

        if self.state.claim_prompt(request.clone()) {
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                windows::open_prompt(&app, &request).await;
            });
        }
        receiver
    }

    /// A Stop is an event, not a decision. tech.md 6.2.
    ///
    /// It closes the rows the turn left open and drops the working status.
    /// Nothing is held and nothing is carried: text reaches an owned session
    /// through its pty the moment it is typed.
    fn on_stop(&self, payload: &Value) {
        let session = peekle_core::sessions::session_ref_of(payload);
        let at = now_ms();

        // Nothing can still be in flight once the turn is over. Whatever is
        // still Running never reported success, because PostToolUse does not
        // fire for a failed call. tech.md 6.3.
        self.state.end_turn(&session.session_id, at);
        self.state
            .set_session_status(&session, SessionStatus::Idle, at);

        // What the agent said last belongs in the feed: the user scrolls back
        // to it. tech.md S6.
        if let Some(text) = payload
            .get("last_assistant_message")
            .and_then(Value::as_str)
            .filter(|t| !t.trim().is_empty())
        {
            let text = peekle_core::truncate(text, peekle_core::LAST_MESSAGE_LIMIT);
            self.state.assistant_turn(&session, &text, at);
        }
        self.emit_sessions(self.state.sessions());

        // The notch opens on the turn it belongs to, and only for a session
        // the island owns: an observed one is read in the user's editor, and
        // surfacing there on every turn boundary is noise. tech.md 6.2.
        if self.state.owns_session(&session.session_id) {
            let app = self.app.clone();
            let session_id = session.session_id.clone();
            tauri::async_runtime::spawn(async move {
                windows::reveal_turn(&app, &session_id).await;
            });
        }

        tracing::debug!(session = %session.session_id, "the turn ended");
    }

    fn prompt_timeout(&self) -> Duration {
        self.state.prompt_timeout()
    }

    /// The router gave up waiting and already answered Claude Code as
    /// `TimedOut`. Settles the same way an explicit answer from the island
    /// would, through the very same function, so the panel and the queue do
    /// not go on believing a decision is still open after the agent has
    /// stopped waiting for one. tech.md rule 10.
    fn settle_timeout(&self, id: &str) {
        crate::commands::settle(&self.app, &self.state, id, PromptOutcome::TimedOut);
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

                // For a session Peekle owns, the process exiting is what marks
                // the card ended, and it says so from the pty. SessionEnd fires
                // at the end of any run while the client behind it carries on,
                // so treating it as the end of an owned session is exactly the
                // guess that used to kill a live input field. tech.md 6.3.
                let cards = if self.state.owns_session(session_id) {
                    tracing::debug!(session = %session_id, "a run ended, the session we own has not");
                    self.state.sessions()
                } else {
                    self.state.mark_session_ended(session_id, at)
                };
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
