//! The seam between the hook endpoints and the rest of the app.
//!
//! The router owns no state. It parses a hook, asks the sink for a decision
//! channel, and turns the outcome into a response body. That keeps the whole
//! HTTP contract of section 6.2 testable without a running Tauri app.

use std::time::Duration;

use peekle_core::types::{PromptOutcome, PromptRequest};
use serde_json::Value;
use tokio::sync::oneshot;

/// What `/stop` should do for one session, one variant per row of the mapping
/// table in tech.md 6.2.
///
/// A Stop is not a question any more. It is the only injection point a session
/// without a tmux pane has, so the only decision is whether keeping it open
/// buys anything.
pub enum StopPlan {
    /// Answer `200 {}` now. Either the text has a faster way in, or there is
    /// nobody left to wait for.
    Release,
    /// The queue already had text when the turn ended. It leaves on this hook.
    Answer(String),
    /// Hold the turn: this is the only channel. Resolves with queued text, or
    /// with None when the sink gives up first.
    Hold(oneshot::Receiver<Option<String>>),
}

pub trait HookSink: Send + Sync + 'static {
    /// When Peekle is off, every blocking endpoint answers `200 {}` before any
    /// window work happens. tech.md section 8.
    fn is_enabled(&self) -> bool;

    /// Registers a pending request and returns the channel it resolves on.
    /// The registry, not the router, owns the resolve-exactly-once invariant.
    fn open_prompt(&self, request: PromptRequest) -> oneshot::Receiver<PromptOutcome>;

    /// Decides whether this turn is worth holding open. Updates the feed and
    /// the session status on the way through, because a Stop is an event
    /// whatever the answer turns out to be. tech.md 6.5.
    fn on_stop(&self, payload: &Value) -> StopPlan;

    /// Held below the hook timeout so Peekle always answers first.
    fn prompt_timeout(&self) -> Duration;

    /// How long a held turn waits for something to be typed. Expiring only
    /// ends the pause, never the channel: text typed later rides out on the
    /// next Stop. tech.md 6.8.
    fn reply_window(&self) -> Duration;

    /// Non-blocking events. These never hold up a turn, so they take the raw
    /// payload and return nothing.
    fn on_feed(&self, payload: &Value);
    fn on_session(&self, payload: &Value);
    fn on_notification(&self, payload: &Value);
}
