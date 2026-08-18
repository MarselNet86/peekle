//! Panel show and hide orchestration. Rust owns the timing: the webview never
//! decides when a window appears or disappears. tech.md section 8.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::types::{PromptOutcome, PromptRequest, ToastRequest};
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

fn show(app: &AppHandle, label: &'static str) {
    on_main(app, "show", move |handle| {
        if let Err(err) = panel::show(handle, label) {
            tracing::error!(error = %err, label, "failed to show a panel");
        }
    });
}

fn hide(app: &AppHandle, label: &'static str) {
    on_main(app, "hide", move |handle| {
        if let Err(err) = panel::hide(handle, label) {
            tracing::error!(error = %err, label, "failed to hide a panel");
        }
    });
}

/// Emits the request, waits for the webview, then shows the panel. The wait is
/// a courtesy, not a gate: if it lapses the panel is shown anyway, because a
/// late frame is better than a hook that never gets an answer.
///
/// The island has no prompt UI until S3, so the request is announced and the
/// pending hook is left to its timeout. Showing an empty black shape for ten
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

/// Shows the island for the toast lifetime, then hides it again. The island is
/// output only, so nothing here waits on the user.
pub fn toast(app: &AppHandle, request: ToastRequest) {
    let ttl = Duration::from_millis(u64::from(request.ttl_ms));

    if let Err(err) = app.emit_to(panel::ISLAND, events::TOAST, &request) {
        tracing::warn!(error = %err, "failed to emit toast");
        return;
    }
    show(app, panel::ISLAND);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(ttl).await;
        hide(&handle, panel::ISLAND);
    });
}
