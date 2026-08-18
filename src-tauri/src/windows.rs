//! Panel show and hide orchestration. Rust owns the timing: the webview never
//! decides when a window appears or disappears. tech.md section 8.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::types::{IslandView, PromptOutcome, PromptRequest, ToastRequest, ToastTone};
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

/// Hands the webview the notch of the display the island is on. Cheap enough
/// to repeat on every open, and the only alternative is reloading the route,
/// which throws away the feed and whatever the user had typed. tech.md 6.7.
pub fn send_notch(app: &AppHandle) {
    let (height, width) = panel::active_notch(app).unwrap_or((0.0, 0.0));
    let payload = serde_json::json!({ "height": height, "width": width });

    if let Err(err) = app.emit_to(panel::ISLAND, events::NOTCH, payload) {
        tracing::warn!(error = %err, "failed to emit notch");
    }
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
    let opening = !matches!(view, IslandView::Collapsed);

    on_main(app, "view", move |handle| {
        if let Err(err) = panel::set_takes_clicks(handle, takes_clicks) {
            tracing::error!(error = %err, "failed to switch cursor events");
        }

        // Re-order on every open. Section 8 forbids hiding the panel between
        // events, not re-asserting its order: the window server decides space
        // membership when a window is ordered front, so a panel ordered once at
        // startup never follows the user onto a full screen space.
        if opening {
            if let Err(err) = panel::show(handle, panel::ISLAND) {
                tracing::error!(error = %err, "failed to raise the island");
            }
            // The island may have just landed on a different display, and the
            // shape has to grow out of that display's bezel. tech.md 6.7.
            send_notch(handle);
        }
    });
}

/// How long the shape holds after an answer before collapsing. tech.md S3.
const COLLAPSE_AFTER: Duration = Duration::from_millis(120);

/// Announces the request, waits for the webview, then opens the island on the
/// session the prompt belongs to.
///
/// The island opens passively: the panel takes the mouse but never the
/// keyboard, so a turn that ends while the user is watching something does not
/// pull the keys out from under them. Focus moves only when they click the
/// field. tech.md 6.7 and 15.
pub async fn open_prompt(app: &AppHandle, request: &PromptRequest) {
    let gate = app.state::<Arc<AppState>>().ready_gate(panel::ISLAND);

    if let Err(err) = app.emit_to(panel::ISLAND, events::PROMPT_OPEN, request) {
        tracing::warn!(error = %err, "failed to emit prompt-open");
    }

    let _ = tokio::time::timeout(READY_TIMEOUT, gate.notified()).await;
    set_view(app, IslandView::Session(request.session.session_id.clone()));
}

/// Tells the webview which outcome settled the request and collapses the
/// island behind it.
pub fn close_prompt(app: &AppHandle, prompt_id: &str, outcome: &PromptOutcome) {
    let payload = serde_json::json!({ "prompt_id": prompt_id, "outcome": outcome });
    if let Err(err) = app.emit_to(panel::ISLAND, events::PROMPT_CLOSE, payload) {
        tracing::warn!(error = %err, "failed to emit prompt-close");
    }

    // Long enough to read as an answer landing, short enough not to be a wait.
    // Skipped when another prompt is already queued behind this one: collapsing
    // and reopening in the same breath reads as a glitch.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(COLLAPSE_AFTER).await;
        let state = handle.state::<Arc<AppState>>().inner().clone();
        if state.active_prompt().is_none() {
            set_view(&handle, IslandView::Collapsed);
        }
    });
}

/// One warning on startup when the combination could not be taken. Swallowing
/// it would leave the user pressing a key that does nothing. tech.md 6.9.
pub fn warn_hotkey(app: &AppHandle, text: &str) {
    toast(
        app,
        ToastRequest {
            text: text.to_string(),
            tone: ToastTone::Warn,
            ttl_ms: 3200,
            badge: None,
        },
    );
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
