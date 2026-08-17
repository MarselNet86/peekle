//! Golden payload tests. Captured Claude Code hooks go into the real axum
//! router and the response is checked field by field against tech.md 6.2.
//!
//! Section 10 calls these the most valuable tests in the project, and the
//! reason is narrow: nothing else here verifies what Claude Code actually
//! sends. Every other test checks our own shapes against our own assumptions.

// A panic in a test is a failed test. The production ban on unwrap is enforced
// by the workspace lint everywhere else.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use peekle_core::types::{PromptAnswer, PromptOutcome, PromptRequest};
use peekle_server::routes::ServerState;
use peekle_server::{router, HookSink};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/hooks")
}

/// Reads one captured payload. A missing file fails loudly: fixtures are
/// captured, never written, so an absent one means the contract is unverified
/// rather than fine. tech.md rule 6.
fn payload(name: &str) -> Value {
    let path = fixture_dir().join(format!("{name}.jsonl"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("capture {name}.jsonl with scripts/capture-hooks.sh"));
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("{name}.jsonl is empty"));
    serde_json::from_str(line).unwrap()
}

struct FixtureSink {
    reply: Option<PromptOutcome>,
    held: Mutex<Vec<oneshot::Sender<PromptOutcome>>>,
    seen: Mutex<Vec<PromptRequest>>,
    feeds: Mutex<Vec<Value>>,
}

impl FixtureSink {
    fn new(reply: Option<PromptOutcome>) -> Arc<Self> {
        Arc::new(Self {
            reply,
            held: Mutex::new(Vec::new()),
            seen: Mutex::new(Vec::new()),
            feeds: Mutex::new(Vec::new()),
        })
    }
}

impl HookSink for FixtureSink {
    fn is_enabled(&self) -> bool {
        true
    }

    fn open_prompt(&self, request: PromptRequest) -> oneshot::Receiver<PromptOutcome> {
        self.seen.lock().unwrap().push(request);
        let (tx, rx) = oneshot::channel();
        match self.reply.clone() {
            Some(outcome) => {
                let _ = tx.send(outcome);
            }
            None => self.held.lock().unwrap().push(tx),
        }
        rx
    }

    fn prompt_timeout(&self) -> Duration {
        Duration::from_millis(80)
    }

    fn on_tasks(&self, payload: &Value) {
        self.feeds.lock().unwrap().push(payload.clone());
    }

    fn on_session(&self, payload: &Value) {
        self.feeds.lock().unwrap().push(payload.clone());
    }

    fn on_notification(&self, payload: &Value) {
        self.feeds.lock().unwrap().push(payload.clone());
    }
}

async fn post(sink: Arc<dyn HookSink>, endpoint: &str, body: &Value) -> (StatusCode, Value) {
    let app = router(ServerState {
        sink,
        token: TOKEN.to_string(),
        version: "0.1.0".to_string(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/h/{TOKEN}/{endpoint}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The fields our mapping reads. If a capture stops carrying one of them,
/// Claude Code changed the contract and tech.md section 6 is what moves.
#[tokio::test]
async fn the_stop_capture_still_carries_the_fields_we_read() {
    let captured = payload("stop");

    assert_eq!(captured["hook_event_name"], "Stop");
    assert!(captured["session_id"].is_string(), "session_id");
    assert!(captured["cwd"].is_string(), "cwd");
    assert!(
        captured["last_assistant_message"].is_string(),
        "last_assistant_message is what the panel shows"
    );
}

#[tokio::test]
async fn a_captured_stop_answered_with_text_blocks_with_that_text() {
    let sink = FixtureSink::new(Some(PromptOutcome::Answered(PromptAnswer {
        prompt_id: "p".to_string(),
        choice: None,
        text: Some("run the tests".to_string()),
    })));

    let (status, body) = post(sink, "stop", &payload("stop")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"decision": "block", "reason": "run the tests"})
    );
}

#[tokio::test]
async fn a_captured_stop_left_unanswered_ends_the_turn_normally() {
    let (status, body) = post(FixtureSink::new(None), "stop", &payload("stop")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}

/// The panel title and message are built from the capture, not from a payload
/// we invented, so this pins the mapping to reality.
#[tokio::test]
async fn a_captured_stop_builds_a_renderable_prompt() {
    let sink = FixtureSink::new(Some(PromptOutcome::Dismissed));
    let handle = Arc::clone(&sink);

    post(sink, "stop", &payload("stop")).await;

    let seen = handle.seen.lock().unwrap();
    let request = seen.first().expect("a prompt was opened");
    assert_eq!(request.title, "Claude finished");
    assert!(
        request.last_message.is_some(),
        "last message reached the panel"
    );
    assert!(!request.session.project.is_empty(), "project name");
    assert!(request.allow_free_text);
}

#[tokio::test]
async fn the_tasks_capture_is_a_todowrite_payload_we_can_read() {
    let captured = payload("tasks");

    assert_eq!(captured["hook_event_name"], "PostToolUse");
    assert_eq!(
        captured["tool_name"], "TodoWrite",
        "the matcher of section 6.1 keeps other tools off this endpoint"
    );

    let todos = captured["tool_input"]["todos"]
        .as_array()
        .expect("tool_input.todos is what the hud is built from");
    assert!(!todos.is_empty());

    for todo in todos {
        assert!(todo["content"].is_string(), "todo content");
        assert!(todo["status"].is_string(), "todo status");
    }
}

#[tokio::test]
async fn a_captured_todowrite_answers_empty_and_reaches_the_sink() {
    let sink = FixtureSink::new(None);
    let handle = Arc::clone(&sink);

    let (status, body) = post(sink, "tasks", &payload("tasks")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
    assert_eq!(handle.feeds.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn the_session_capture_names_its_event() {
    let captured = payload("session");
    let name = captured["hook_event_name"].as_str().expect("event name");

    assert!(
        name == "SessionStart" || name == "SessionEnd",
        "unexpected session event {name}"
    );
    assert!(captured["session_id"].is_string());
}

#[tokio::test]
async fn a_captured_session_event_answers_empty() {
    let (status, body) = post(FixtureSink::new(None), "session", &payload("session")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}

/// Captures carry fields no build knows about yet. Dropping one must not turn
/// into a rejected hook. tech.md 6.2.
#[tokio::test]
async fn unknown_captured_fields_pass_through_without_complaint() {
    let mut captured = payload("stop");
    captured["a_field_from_a_future_release"] = json!({"nested": [1, 2, 3]});

    let (status, _) = post(FixtureSink::new(None), "stop", &captured).await;
    assert_eq!(status, StatusCode::OK);
}
