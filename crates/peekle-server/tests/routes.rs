//! HTTP contract tests. Every assertion traces to a row of tech.md 6.2.
//!
//! Payloads here are minimal on purpose: they exercise routing, status codes
//! and our own response shapes. Golden payloads captured from a live Claude
//! Code session live in `fixtures/hooks/` and are asserted separately.

// A panic in a test is a failed test, which is the point. The production ban
// on unwrap is enforced by the workspace lint everywhere else.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use peekle_core::types::{PromptAnswer, PromptOutcome, PromptRequest};
use peekle_server::routes::ServerState;
use peekle_server::{router, HookSink, StopPlan, CORE_VERSION};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// Answers every prompt with a canned outcome, and records what it was asked.
struct TestSink {
    enabled: bool,
    timeout: Duration,
    reply: Mutex<Option<PromptOutcome>>,
    seen: Mutex<Vec<PromptRequest>>,
    feeds: Mutex<Vec<(&'static str, Value)>>,
    /// Senders for prompts this sink deliberately never answers. Holding them
    /// keeps the receiver waiting instead of erroring out early.
    held: Mutex<Vec<oneshot::Sender<PromptOutcome>>>,
    /// What `/stop` should decide. None means release straight away.
    stop: Mutex<StopBehaviour>,
    /// Senders for parked turns, held for the same reason as `held`.
    parked: Mutex<Vec<oneshot::Sender<Option<String>>>>,
}

/// What the sink under test tells `/stop` to do. One variant per row of the
/// mapping table in tech.md 6.2.
#[derive(Clone)]
enum StopBehaviour {
    /// There is another channel, or nobody to wait for.
    Release,
    /// The queue already had text when the turn ended.
    Answer(String),
    /// Park the turn and never resolve it, so the window has to expire.
    HoldForever,
}

impl TestSink {
    fn new(reply: Option<PromptOutcome>) -> Arc<Self> {
        Arc::new(Self {
            enabled: true,
            timeout: Duration::from_secs(5),
            reply: Mutex::new(reply),
            seen: Mutex::new(Vec::new()),
            feeds: Mutex::new(Vec::new()),
            held: Mutex::new(Vec::new()),
            stop: Mutex::new(StopBehaviour::Release),
            parked: Mutex::new(Vec::new()),
        })
    }

    /// A sink whose `/stop` behaves the given way.
    fn stopping(behaviour: StopBehaviour) -> Arc<Self> {
        let sink = Self::new(None);
        *sink.stop.lock().unwrap() = behaviour;
        sink
    }

    fn disabled() -> Arc<Self> {
        Arc::new(Self {
            enabled: false,
            timeout: Duration::from_secs(5),
            reply: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
            feeds: Mutex::new(Vec::new()),
            held: Mutex::new(Vec::new()),
            stop: Mutex::new(StopBehaviour::Release),
            parked: Mutex::new(Vec::new()),
        })
    }

    /// Never answers, so the endpoint has to fall back to its own timeout.
    fn silent() -> Arc<Self> {
        Arc::new(Self {
            enabled: true,
            timeout: Duration::from_millis(80),
            reply: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
            feeds: Mutex::new(Vec::new()),
            held: Mutex::new(Vec::new()),
            stop: Mutex::new(StopBehaviour::HoldForever),
            parked: Mutex::new(Vec::new()),
        })
    }
}

impl HookSink for TestSink {
    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn open_prompt(&self, request: PromptRequest) -> oneshot::Receiver<PromptOutcome> {
        self.seen.lock().unwrap().push(request);
        let (tx, rx) = oneshot::channel();
        if let Some(outcome) = self.reply.lock().unwrap().clone() {
            let _ = tx.send(outcome);
        } else {
            self.held.lock().unwrap().push(tx);
        }
        rx
    }

    fn on_stop(&self, _payload: &Value) -> StopPlan {
        match self.stop.lock().unwrap().clone() {
            StopBehaviour::Release => StopPlan::Release,
            StopBehaviour::Answer(text) => StopPlan::Answer(text),
            StopBehaviour::HoldForever => {
                let (tx, rx) = oneshot::channel();
                self.parked.lock().unwrap().push(tx);
                StopPlan::Hold(rx)
            }
        }
    }

    fn reply_window(&self) -> Duration {
        self.timeout
    }

    fn prompt_timeout(&self) -> Duration {
        self.timeout
    }

    fn on_feed(&self, payload: &Value) {
        self.feeds.lock().unwrap().push(("feed", payload.clone()));
    }

    fn on_session(&self, payload: &Value) {
        self.feeds
            .lock()
            .unwrap()
            .push(("session", payload.clone()));
    }

    fn on_notification(&self, payload: &Value) {
        self.feeds
            .lock()
            .unwrap()
            .push(("notification", payload.clone()));
    }
}

fn app(sink: Arc<dyn HookSink>) -> axum::Router {
    router(ServerState {
        sink,
        token: TOKEN.to_string(),
        version: "0.1.0".to_string(),
    })
}

async fn post(app: axum::Router, path: &str, body: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn answered(choice: Option<&str>, text: Option<&str>) -> PromptOutcome {
    PromptOutcome::Answered(PromptAnswer {
        prompt_id: "p".to_string(),
        choice: choice.map(str::to_owned),
        text: text.map(str::to_owned),
    })
}

#[tokio::test]
async fn health_reports_version_enabled_and_core() {
    let response = app(TestSink::new(None))
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["version"], "0.1.0");
    assert_eq!(body["enabled"], true);
    assert_eq!(body["core"], CORE_VERSION);
}

#[tokio::test]
async fn a_wrong_token_is_a_404_with_no_body() {
    for path in ["stop", "permission", "notification", "feed", "session"] {
        let uri = format!("/v1/h/deadbeef/{path}");
        let (status, body) = post(app(TestSink::new(None)), &uri, "{}").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body, Value::Null, "{path}");
    }
}

#[tokio::test]
async fn a_body_that_is_not_json_is_a_400() {
    for path in ["stop", "permission", "notification", "feed", "session"] {
        let uri = format!("/v1/h/{TOKEN}/{path}");
        let (status, _) = post(app(TestSink::new(None)), &uri, "not json at all").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
}

/// Text already queued when the turn ended leaves on that very Stop.
#[tokio::test]
async fn queued_text_comes_back_as_a_block_decision() {
    let sink = TestSink::stopping(StopBehaviour::Answer("run the tests".into()));
    let (status, body) = post(app(sink), &format!("/v1/h/{TOKEN}/stop"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"decision": "block", "reason": "run the tests"})
    );
}

/// A session with a tmux pane is let go at once: its text goes in as
/// keystrokes whenever it is typed, so parking the turn buys nothing.
#[tokio::test]
async fn a_session_with_another_channel_is_released_at_once() {
    let sink = TestSink::stopping(StopBehaviour::Release);
    let (status, body) = post(app(sink), &format!("/v1/h/{TOKEN}/stop"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}

/// A held turn whose window passes ends normally. The text is not lost by
/// this: it stays queued for the next Stop, which is the sink's business.
/// tech.md 6.8.
#[tokio::test]
async fn a_held_turn_that_nobody_answers_ends_normally() {
    let sink = TestSink::silent();
    let (status, body) = post(app(sink), &format!("/v1/h/{TOKEN}/stop"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}

/// Stop never registers a pending request any more: it is not a question.
#[tokio::test]
async fn stop_opens_no_prompt() {
    let sink = TestSink::stopping(StopBehaviour::Answer("go".into()));
    let handle = Arc::clone(&sink);
    let _ = post(
        app(Arc::clone(&sink) as Arc<dyn HookSink>),
        &format!("/v1/h/{TOKEN}/stop"),
        "{}",
    )
    .await;
    assert!(handle.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn permission_allow_carries_the_documented_envelope() {
    let sink = TestSink::new(Some(answered(Some("allow_once"), None)));
    let (status, body) = post(
        app(sink),
        &format!("/v1/h/{TOKEN}/permission"),
        r#"{"tool_name":"Bash"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": {"behavior": "allow"}
        }})
    );
}

/// S8. Off short circuits the blocking endpoints while the feed and the
/// session registry keep collecting, so switching mid run changes what the
/// agent is told on the very next hook without a restart. tech.md 6.9 and 8.
#[tokio::test]
async fn a_disabled_peekle_keeps_collecting_while_it_stops_deciding() {
    let sink = TestSink::disabled();
    let handle = Arc::clone(&sink);

    for path in ["notification", "feed", "session"] {
        let uri = format!("/v1/h/{TOKEN}/{path}");
        let (status, body) = post(
            app(Arc::clone(&sink) as Arc<dyn HookSink>),
            &uri,
            r#"{"hook_event_name":"PreToolUse","session_id":"s"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body, json!({}), "{path}");
    }

    assert_eq!(
        handle.feeds.lock().unwrap().len(),
        3,
        "bypass stops decisions, not collection"
    );
    assert!(handle.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_disabled_peekle_answers_empty_without_opening_a_prompt() {
    let sink = TestSink::disabled();
    let handle = Arc::clone(&sink);

    for path in ["stop", "permission"] {
        let uri = format!("/v1/h/{TOKEN}/{path}");
        let (status, body) = post(app(Arc::clone(&sink) as Arc<dyn HookSink>), &uri, "{}").await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body, json!({}), "{path}");
    }
    assert!(
        handle.seen.lock().unwrap().is_empty(),
        "bypass must not register a pending request"
    );
}

#[tokio::test]
async fn a_prompt_that_is_never_answered_times_out_into_an_empty_body() {
    let sink = TestSink::silent();
    let (status, body) = post(app(sink), &format!("/v1/h/{TOKEN}/permission"), "{}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}

#[tokio::test]
async fn feeds_answer_empty_and_hand_the_payload_over() {
    let sink = TestSink::new(None);
    let handle = Arc::clone(&sink);

    for path in ["notification", "feed", "session"] {
        let uri = format!("/v1/h/{TOKEN}/{path}");
        let (status, body) = post(
            app(Arc::clone(&sink) as Arc<dyn HookSink>),
            &uri,
            r#"{"hook_event_name":"x"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body, json!({}), "{path}");
    }

    let feeds = handle.feeds.lock().unwrap();
    assert_eq!(feeds.len(), 3);
}

#[tokio::test]
async fn unknown_payload_fields_are_ignored_rather_than_rejected() {
    let sink = TestSink::stopping(StopBehaviour::Release);
    let body = r#"{"session_id":"s","cwd":"/tmp","brand_new_field":{"nested":[1,2]}}"#;
    let (status, body) = post(app(sink), &format!("/v1/h/{TOKEN}/stop"), body).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}
