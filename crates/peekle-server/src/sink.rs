//! The seam between the hook endpoints and the rest of the app.
//!
//! The router owns no state. It parses a hook, asks the sink for a decision
//! channel, and turns the outcome into a response body. That keeps the whole
//! HTTP contract of section 6.2 testable without a running Tauri app.

use std::time::Duration;

use peekle_core::types::{PromptOutcome, PromptRequest};
use serde_json::Value;
use tokio::sync::oneshot;

pub trait HookSink: Send + Sync + 'static {
    /// When Peekle is off, every blocking endpoint answers `200 {}` before any
    /// window work happens. tech.md section 8.
    fn is_enabled(&self) -> bool;

    /// Registers a pending request and returns the channel it resolves on.
    /// The registry, not the router, owns the resolve-exactly-once invariant.
    fn open_prompt(&self, request: PromptRequest) -> oneshot::Receiver<PromptOutcome>;

    /// The turn ended. Closes the feed rows it left open and drops the working
    /// status. Never blocks and never returns a decision: text reaches an
    /// owned session through its pty, so there is nothing to wait for and
    /// nothing to carry. tech.md 6.2 and 6.5.
    fn on_stop(&self, payload: &Value);

    /// Held below the hook timeout so Peekle always answers first.
    fn prompt_timeout(&self) -> Duration;

    /// Non-blocking events. These never hold up a turn, so they take the raw
    /// payload and return nothing.
    fn on_feed(&self, payload: &Value);
    fn on_session(&self, payload: &Value);
    fn on_notification(&self, payload: &Value);
}
