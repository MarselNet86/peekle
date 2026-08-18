//! Panel show and hide orchestration. Rust owns the timing: the webview never
//! decides when a window appears or disappears. tech.md section 8.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::types::{IslandView, PromptOutcome, PromptRequest, ToastRequest};
use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::panel;
use crate::state::AppState;

/// How long Rust waits for the webview to report it painted the route.
/// Showing before the content exists gives a visible empty box.
const READY_TIMEOUT: Duration = Duration::from_millis(250);

/// Every AppKit call has to run on the main thread. Hooks arrive on axum's
/// worker threads, so nothing in this file touches a panel directly.
fn on_main<F>(app: &AppHandle, what: &'static str, work: F)
where
    F: FnOnce(&AppHandle) + Send + 'static,
{
    let handle = app.clone();
    if let Err(err) = app.run_on_main_thread(move || work(&handle)) {
        tracing::error!(error = %err, what, "could not reach the main thread");
    }
}

/// Puts the island on screen once and leaves it there. It is transparent while
/// collapsed, and hiding the window on every collapse would flash and cut the
/// spring short. tech.md section 8.
pub async fn open_island(app: &AppHandle) {
    let gate = app.state::<Arc<AppState>>().ready_gate(panel::ISLAND);
    let _ = tokio::time::timeout(READY_TIMEOUT, gate.notified()).await;

    on_main(app, "show", move |handle| {
        if let Err(err) = panel::show(handle, panel::ISLAND) {
            tracing::error!(error = %err, "failed to show the island");
        }
    });
}

/// The one way the island changes shape. Stores the intent, tells the webview
/// to redraw, and switches mouse handling to match. A view that has not moved
/// does nothing at all.
pub fn set_view(app: &AppHandle, view: IslandView) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    if !state.set_view(view.clone()) {
        return;
    }

    if let Err(err) = app.emit_to(panel::ISLAND, events::VIEW, &view) {
        tracing::warn!(error = %err, "failed to emit view");
    }

    let takes_clicks = view.takes_clicks();
    on_main(app, "cursor", move |handle| {
        if let Err(err) = panel::set_takes_clicks(handle, takes_clicks) {
            tracing::error!(error = %err, "failed to switch cursor events");
        }
    });
}

/// Emits the request and waits for the webview.
///
/// The island has no prompt UI until S3, so the request is announced and the
/// pending hook is left to its timeout. Growing an empty black shape for ten
/// minutes would be worse than showing nothing.
pub async fn open_prompt(app: &AppHandle, request: &PromptRequest) {
    let gate = app.state::<Arc<AppState>>().ready_gate(panel::ISLAND);

    if let Err(err) = app.emit_to(panel::ISLAND, events::PROMPT_OPEN, request) {
        tracing::warn!(error = %err, "failed to emit prompt-open");
    }

    let _ = tokio::time::timeout(READY_TIMEOUT, gate.notified()).await;
}

/// Tells the webview which outcome settled the request.
pub fn close_prompt(app: &AppHandle, prompt_id: &str, outcome: &PromptOutcome) {
    let payload = serde_json::json!({ "prompt_id": prompt_id, "outcome": outcome });
    if let Err(err) = app.emit_to(panel::ISLAND, events::PROMPT_CLOSE, payload) {
        tracing::warn!(error = %err, "failed to emit prompt-close");
    }
}

/// A toast is the `Pill` view for as long as it lives. Nothing here waits on
/// the user, so the island collapses itself when the time is up.
pub fn toast(app: &AppHandle, request: ToastRequest) {
    let ttl = Duration::from_millis(u64::from(request.ttl_ms));

    if let Err(err) = app.emit_to(panel::ISLAND, events::TOAST, &request) {
        tracing::warn!(error = %err, "failed to emit toast");
        return;
    }
    set_view(app, IslandView::Pill);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(ttl).await;

        // Something more important may have opened in the meantime. Collapsing
        // then would throw away a session the user is reading.
        let state = handle.state::<Arc<AppState>>().inner().clone();
        if state.view() == IslandView::Pill {
            set_view(&handle, IslandView::Collapsed);
        }
    });
}
