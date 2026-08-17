//! Wires the hook server to the app. Implements `HookSink` from peekle-server.
//!
//! Every field lookup here is optional. Claude Code adds fields between
//! versions, and the exact task payloads are pinned by captured fixtures
//! rather than by documentation. tech.md R-4.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::labels::classify;
use peekle_core::types::{
    PromptOutcome, PromptRequest, TaskItem, TaskStatus, ToastRequest, ToastTone,
};
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

    fn prompt_timeout(&self) -> Duration {
        self.state.prompt_timeout()
    }

    fn on_tasks(&self, payload: &Value) {
        let items = parse_tasks(payload);
        if items.is_empty() {
            return;
        }
        let merged = self.state.merge_tasks(items);
        if let Err(err) = self.app.emit(events::TASKS, &merged) {
            tracing::warn!(error = %err, "failed to emit tasks");
        }
        windows::sync_hud(&self.app, merged.len());
    }

    fn on_session(&self, payload: &Value) {
        match payload.get("hook_event_name").and_then(Value::as_str) {
            Some("SessionStart") => self.state.session_started(),
            Some("SessionEnd") => self.state.session_ended(),
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
/// an empty list rather than a partial one, so the HUD never shows debris.
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
