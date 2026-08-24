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
    /// The payloads `/stop` handed over, which is how the feed learns a turn
    /// ended now that Stop opens no prompt.
    stops: Mutex<Vec<Value>>,
}

impl FixtureSink {
    fn new(reply: Option<PromptOutcome>) -> Arc<Self> {
        Arc::new(Self {
            reply,
            held: Mutex::new(Vec::new()),
            seen: Mutex::new(Vec::new()),
            feeds: Mutex::new(Vec::new()),
            stops: Mutex::new(Vec::new()),
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

    fn on_stop(&self, payload: &Value) {
        self.stops.lock().unwrap().push(payload.clone());
    }

    /// Not exercised here: the fixture contract tests never let a prompt run
    /// long enough to time out.
    fn settle_timeout(&self, _id: &str) {}

    fn prompt_timeout(&self) -> Duration {
        Duration::from_millis(80)
    }

    fn on_feed(&self, payload: &Value) {
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

/// The real captured Stop gets `200 {}` and nothing else. It carries no text
/// and holds no turn: an owned session already took the reply through its pty.
/// tech.md 6.2.
#[tokio::test]
async fn a_captured_stop_ends_the_turn_and_decides_nothing() {
    let (status, body) = post(FixtureSink::new(None), "stop", &payload("stop")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}

/// Stop is an event, not a question: it opens no prompt and the payload
/// reaches the sink so the feed can close the turn. tech.md 6.5.
#[tokio::test]
async fn a_captured_stop_opens_no_prompt_and_reaches_the_sink() {
    let sink = FixtureSink::new(None);
    let handle = Arc::clone(&sink);

    post(sink, "stop", &payload("stop")).await;

    assert!(
        handle.seen.lock().unwrap().is_empty(),
        "Stop must not register a pending request"
    );
    let stops = handle.stops.lock().unwrap();
    let seen = stops.first().expect("the payload reached the sink");
    assert_eq!(seen["hook_event_name"], "Stop");
    assert!(
        seen["last_assistant_message"].is_string(),
        "the last message is there for the feed"
    );
}

#[tokio::test]
async fn the_tasks_capture_is_a_todowrite_payload_we_can_read() {
    let captured = payload("tasks");

    assert_eq!(captured["hook_event_name"], "PostToolUse");
    assert_eq!(captured["tool_name"], "TodoWrite");

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

    let (status, body) = post(sink, "feed", &payload("tasks")).await;

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

#[tokio::test]
async fn the_user_prompt_capture_carries_the_turn_itself() {
    let captured = payload("user_prompt_submit");

    assert_eq!(captured["hook_event_name"], "UserPromptSubmit");
    assert!(
        captured["prompt"].is_string(),
        "the prompt is what the feed shows and what titles the session"
    );
    assert!(captured["session_id"].is_string());
    assert!(captured["cwd"].is_string());
}

#[tokio::test]
async fn the_pre_tool_capture_names_the_call_it_opens() {
    let captured = payload("pre_tool_use");

    assert_eq!(captured["hook_event_name"], "PreToolUse");
    assert!(captured["tool_name"].is_string());
    assert!(captured["tool_input"].is_object());
    assert!(
        captured["tool_use_id"].is_string(),
        "without tool_use_id the row cannot be closed later"
    );
}

#[tokio::test]
async fn the_post_tool_capture_closes_a_call_by_the_same_id() {
    let captured = payload("post_tool_use");

    assert_eq!(captured["hook_event_name"], "PostToolUse");
    assert!(captured["tool_use_id"].is_string());
    assert!(
        captured["tool_response"].is_object() || captured["tool_response"].is_string(),
        "a closed call reports something back"
    );
}

/// The v8 finding, pinned so a Claude Code update that starts reporting
/// failures shows up here rather than in a user's feed. tech.md 6.1.
#[tokio::test]
async fn no_captured_post_tool_use_reports_a_failure() {
    let path = fixture_dir().join("post_tool_use.jsonl");
    let text = std::fs::read_to_string(&path).expect("capture post_tool_use.jsonl");

    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let captured: Value = serde_json::from_str(line).unwrap();
        let response = &captured["tool_response"];

        assert!(
            !response["is_error"].as_bool().unwrap_or(false),
            "a captured PostToolUse reported an error, so 6.3 can stop inferring at the turn boundary"
        );
        assert!(
            !response["interrupted"].as_bool().unwrap_or(false),
            "a captured PostToolUse reported an interruption"
        );
    }
}

#[tokio::test]
async fn every_feed_event_answers_empty_and_reaches_the_sink() {
    for name in ["user_prompt_submit", "pre_tool_use", "post_tool_use"] {
        let sink = FixtureSink::new(None);
        let handle = Arc::clone(&sink);

        let (status, body) = post(sink, "feed", &payload(name)).await;

        assert_eq!(status, StatusCode::OK, "{name}");
        assert_eq!(body, json!({}), "{name} must never carry a decision");
        assert_eq!(handle.feeds.lock().unwrap().len(), 1, "{name}");
    }
}

/// S4. Every behavior of the 6.2 permission table, driven by the captured
/// PermissionRequest rather than a payload we imagined.
#[tokio::test]
async fn each_captured_permission_choice_maps_to_its_documented_envelope() {
    for (choice, behavior) in [
        ("allow_once", "allow"),
        ("allow_always", "allow"),
        ("deny", "deny"),
    ] {
        let sink = FixtureSink::new(Some(PromptOutcome::Answered(PromptAnswer {
            prompt_id: "p".to_string(),
            choice: Some(choice.to_string()),
            text: None,
        })));

        let (status, body) = post(sink, "permission", &payload("permission")).await;

        assert_eq!(status, StatusCode::OK, "{choice}");
        assert_eq!(
            body,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {"behavior": behavior},
                }
            }),
            "{choice}"
        );
    }
}

#[tokio::test]
async fn a_captured_permission_denied_with_a_reason_carries_it() {
    let sink = FixtureSink::new(Some(PromptOutcome::Answered(PromptAnswer {
        prompt_id: "p".to_string(),
        choice: Some("deny".to_string()),
        text: Some("  not on this machine  ".to_string()),
    })));

    let (status, body) = post(sink, "permission", &payload("permission")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["hookSpecificOutput"]["decision"],
        json!({"behavior": "deny", "message": "not on this machine"})
    );
}

/// Dismissed, timed out and bypassed all mean no decision. Claude Code falls
/// back to asking in the terminal, which is the safe direction to fail.
#[tokio::test]
async fn a_captured_permission_left_alone_carries_no_decision() {
    for outcome in [
        PromptOutcome::Dismissed,
        PromptOutcome::TimedOut,
        PromptOutcome::Bypassed,
    ] {
        let sink = FixtureSink::new(Some(outcome.clone()));
        let (status, body) = post(sink, "permission", &payload("permission")).await;

        assert_eq!(status, StatusCode::OK, "{outcome:?}");
        assert_eq!(body, json!({}), "{outcome:?}");
    }
}

/// The prompt the island draws is built from the capture, so this pins the
/// title and the input preview to what Claude Code actually sends.
#[tokio::test]
async fn a_captured_permission_builds_a_renderable_prompt() {
    let sink = FixtureSink::new(Some(PromptOutcome::Dismissed));
    let handle = Arc::clone(&sink);

    post(sink, "permission", &payload("permission")).await;

    let seen = handle.seen.lock().unwrap();
    let request = seen.first().expect("a prompt was opened");
    assert_eq!(request.title, "Bash needs permission");
    assert!(
        request
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("echo appended")),
        "the user has to see the command they are agreeing to: {:?}",
        request.detail
    );
    assert!(!request.session.project.is_empty());
}

/// The v11 finding, pinned. If Claude Code starts sending tool_use_id here,
/// the feed could tie a denial straight to its row and 6.3 should say so.
#[tokio::test]
async fn the_permission_capture_still_carries_no_tool_use_id() {
    let captured = payload("permission");

    assert_eq!(captured["hook_event_name"], "PermissionRequest");
    assert!(captured["tool_name"].is_string());
    assert!(captured["tool_input"].is_object());
    assert!(
        captured.get("tool_use_id").is_none(),
        "PermissionRequest now carries tool_use_id, so a denial can be tied to its feed row"
    );
}

#[tokio::test]
async fn the_notification_capture_matches_the_documented_matcher() {
    let captured = payload("notification");

    assert_eq!(captured["hook_event_name"], "Notification");
    assert!(captured["message"].is_string());

    let kind = captured["notification_type"].as_str().unwrap_or_default();
    assert!(
        [
            "permission_prompt",
            "idle_prompt",
            "agent_needs_input",
            "agent_completed"
        ]
        .contains(&kind),
        "the 6.1 matcher would drop {kind}"
    );
}

#[tokio::test]
async fn a_captured_notification_answers_empty() {
    let sink = FixtureSink::new(None);
    let handle = Arc::clone(&sink);

    let (status, body) = post(sink, "notification", &payload("notification")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
    assert_eq!(handle.feeds.lock().unwrap().len(), 1);
}
