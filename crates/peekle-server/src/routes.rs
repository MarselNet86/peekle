//! Axum router. Paths, statuses and bodies come from tech.md section 6.2.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use peekle_core::types::{PromptOutcome, PromptRequest};
use serde_json::{json, Value};
use ulid::Ulid;

use crate::map;
use crate::sink::HookSink;

/// Reported by `/v1/health`. Tracks the core version in the tech.md header.
pub const CORE_VERSION: &str = "v12";

#[derive(Clone)]
pub struct ServerState {
    pub sink: Arc<dyn HookSink>,
    pub token: String,
    pub version: String,
}

pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/h/{token}/stop", post(stop))
        .route("/v1/h/{token}/permission", post(permission))
        .route("/v1/h/{token}/notification", post(notification))
        .route("/v1/h/{token}/feed", post(feed_route))
        .route("/v1/h/{token}/session", post(session))
        .with_state(state)
}

/// `200 {}` is the answer to almost everything: no decision, turn continues as
/// if Peekle were not installed.
fn empty() -> Response {
    (StatusCode::OK, Json(json!({}))).into_response()
}

/// A bad token is indistinguishable from a wrong path on purpose, and the
/// value never reaches a log line.
fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

fn check_token(state: &ServerState, token: &str) -> bool {
    token == state.token
}

/// The body arrives as raw bytes so a malformed payload answers 400 instead of
/// letting axum's own rejection shape leak out.
fn parse(body: &[u8]) -> Result<Value, StatusCode> {
    serde_json::from_slice(body).map_err(|_| StatusCode::BAD_REQUEST)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

async fn health(State(state): State<ServerState>) -> Response {
    Json(json!({
        "version": state.version,
        "enabled": state.sink.is_enabled(),
        "core": CORE_VERSION,
    }))
    .into_response()
}

/// Shared path for both blocking endpoints: build the request, hand it to the
/// sink, wait no longer than the configured timeout, map the outcome.
async fn blocking(
    state: ServerState,
    token: String,
    body: bytes::Bytes,
    build: fn(String, &Value, i64, i64) -> PromptRequest,
    render: fn(&PromptOutcome, &PromptRequest) -> Value,
) -> Response {
    if !check_token(&state, &token) {
        return not_found();
    }
    let payload = match parse(&body) {
        Ok(payload) => payload,
        Err(status) => return status.into_response(),
    };

    // Off short-circuits before any window work. The agent runs in its normal
    // mode and nothing is left pending.
    if !state.sink.is_enabled() {
        return empty();
    }

    let timeout = state.sink.prompt_timeout();
    let now = now_ms();
    let request = build(
        Ulid::generate().to_string(),
        &payload,
        now,
        now + timeout.as_millis() as i64,
    );

    let receiver = state.sink.open_prompt(request.clone());
    let outcome = match tokio::time::timeout(timeout, receiver).await {
        Ok(Ok(outcome)) => outcome,
        // Timed out, or the sender was dropped without an answer. Either way
        // the request is settled and the turn ends normally.
        Ok(Err(_)) | Err(_) => PromptOutcome::TimedOut,
    };

    (StatusCode::OK, Json(render(&outcome, &request))).into_response()
}

async fn stop(
    State(state): State<ServerState>,
    Path(token): Path<String>,
    body: bytes::Bytes,
) -> Response {
    blocking(state, token, body, map::stop_request, map::stop_body).await
}

async fn permission(
    State(state): State<ServerState>,
    Path(token): Path<String>,
    body: bytes::Bytes,
) -> Response {
    blocking(
        state,
        token,
        body,
        map::permission_request,
        map::permission_body,
    )
    .await
}

/// Non-blocking endpoints. They keep collecting while Peekle is off: the feed
/// and the session registry stay alive in bypass mode. tech.md section 8.
async fn non_blocking(
    state: ServerState,
    token: String,
    body: bytes::Bytes,
    deliver: fn(&dyn HookSink, &Value),
) -> Response {
    if !check_token(&state, &token) {
        return not_found();
    }
    match parse(&body) {
        Ok(payload) => {
            deliver(state.sink.as_ref(), &payload);
            empty()
        }
        Err(status) => status.into_response(),
    }
}

async fn notification(
    State(state): State<ServerState>,
    Path(token): Path<String>,
    body: bytes::Bytes,
) -> Response {
    non_blocking(state, token, body, |sink, payload| {
        sink.on_notification(payload)
    })
    .await
}

async fn feed_route(
    State(state): State<ServerState>,
    Path(token): Path<String>,
    body: bytes::Bytes,
) -> Response {
    non_blocking(state, token, body, |sink, payload| sink.on_feed(payload)).await
}

async fn session(
    State(state): State<ServerState>,
    Path(token): Path<String>,
    body: bytes::Bytes,
) -> Response {
    non_blocking(state, token, body, |sink, payload| sink.on_session(payload)).await
}

/// Convenience for callers that want a ready-made timeout.
pub fn default_timeout() -> Duration {
    Duration::from_secs(600)
}
