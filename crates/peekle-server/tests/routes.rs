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
use peekle_server::{router, HookSink, CORE_VERSION};
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
    /// Every `/stop` payload the sink was handed. A Stop is an event now, so
    /// what matters is that it arrived, not what it decided. tech.md 6.2.
    stops: Mutex<Vec<Value>>,
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
            stops: Mutex::new(Vec::new()),
        })
    }

    fn disabled() -> Arc<Self> {
        Arc::new(Self {
            enabled: false,
            timeout: Duration::from_secs(5),
            reply: Mutex::new(None),
            seen: Mutex::new(Vec::new()),
            feeds: Mutex::new(Vec::new()),
            held: Mutex::new(Vec::new()),
            stops: Mutex::new(Vec::new()),
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
            stops: Mutex::new(Vec::new()),
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

    fn on_stop(&self, payload: &Value) {
        self.stops.lock().unwrap().push(payload.clone());
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

/// Stop never blocks and never returns a decision. It is an event: the turn
/// ended. Text reaches an owned session through its pty when it is typed, so
/// there is nothing to carry here and nothing to wait for. tech.md 6.2.
#[tokio::test]
async fn stop_always_answers_empty_and_never_blocks() {
    let sink = TestSink::new(None);
    let handle = Arc::clone(&sink);
    let (status, body) = post(
        app(Arc::clone(&sink) as Arc<dyn HookSink>),
        &format!("/v1/h/{TOKEN}/stop"),
        r#"{"session_id":"s"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}), "no decision on any path");
    assert_eq!(
        handle.stops.lock().unwrap().len(),
        1,
        "the turn ending is still an event the sink sees"
    );
}

/// Stop never registers a pending request any more: it is not a question.
#[tokio::test]
async fn stop_opens_no_prompt() {
    let sink = TestSink::new(None);
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
    let sink = TestSink::new(None);
    let body = r#"{"session_id":"s","cwd":"/tmp","brand_new_field":{"nested":[1,2]}}"#;
    let (status, body) = post(app(sink), &format!("/v1/h/{TOKEN}/stop"), body).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({}));
}
