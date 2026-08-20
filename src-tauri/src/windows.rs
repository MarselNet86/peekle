//! Panel show and hide orchestration. Rust owns the timing: the webview never
//! decides when a window appears or disappears. tech.md section 8.

use std::sync::Arc;
use std::time::{Duration, Instant};

use peekle_core::island::shape_rect;
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

/// How often Rust asks where the pointer is while the island rests.
///
/// The webview cannot answer this: a window that ignores the cursor never sees
/// a `mousemove`, so the only way to know the pointer reached the resting mark
/// is to look. Ten times a second is under the threshold where a user notices
/// the mark lighting up late, and the tick costs a rectangle test.
const HOVER_TICK: Duration = Duration::from_millis(100);

/// Hands the mouse to the island while the pointer is over the resting mark
/// and takes it back the moment it leaves.
///
/// A collapsed island is a transparent 720 by 560 rectangle. Letting it keep
/// the mouse would swallow every click in the top third of the screen, and
/// letting it never take the mouse would make the mark impossible to press.
/// tech.md 6.7.
pub fn track_pointer(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(HOVER_TICK);
        loop {
            ticker.tick().await;
            on_main(&handle, "hover", update_hover);
        }
    });
}

/// How long the pointer has to be off an island the user opened before it puts
/// itself away. Long enough to cross a gap by accident, short enough that the
/// island does not sit on the screen after the user has moved on. tech.md 6.7.
const DISMISS_AFTER: Duration = Duration::from_millis(800);

fn update_hover(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();

    // No measurement yet means no rectangle. Guessing one would eat clicks
    // next to a mark the user cannot even see. tech.md 6.7.
    let Some(bounds) = state.shape_bounds() else {
        return;
    };
    let Ok((frame, scale)) = panel::island_frame(app) else {
        return;
    };
    let Some(rect) = shape_rect(frame, (bounds.0 * scale, bounds.1 * scale)) else {
        return;
    };
    let Ok(pointer) = app.cursor_position() else {
        return;
    };
    let inside = rect.contains((pointer.x, pointer.y));

    if state.view() == IslandView::Collapsed {
        state.pointer_returned();
        if state.set_over_rest(inside) {
            if let Err(err) = panel::set_takes_clicks(app, inside) {
                tracing::error!(error = %err, "failed to switch cursor events for the mark");
            }
        }
        return;
    }

    // An open island already takes the mouse outright, and `set_view` said so.
    state.set_over_rest(false);

    // A request in flight closes on an answer, a dismissal or a timeout, never
    // on the pointer wandering off: that is the resolve exactly once invariant
    // of rule 10. A pill runs on its own clock.
    if state.active_prompt().is_some() || state.view() == IslandView::Pill {
        state.pointer_returned();
        return;
    }

    if inside {
        state.pointer_returned();
        return;
    }
    if state.pointer_left_for(DISMISS_AFTER, Instant::now()) {
        tracing::debug!("the pointer left the island, putting it away");
        set_view(app, IslandView::Collapsed);
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

    // An island that opens on a stale snapshot draws it and refreshes behind
    // itself. It never waits: a slow network must not delay the shape.
    // tech.md 6.4.
    if opening {
        refresh_stale_usage(app, &state);
    }

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

/// How old a snapshot may be when the island opens before it is worth asking
/// again. tech.md 6.4.
const STALE_AFTER_MS: i64 = 60_000;

/// Asks for fresh numbers behind an opening island, and only if the last ones
/// are old. Silent when usage may not be fetched at all: rule 12 keeps every
/// automatic path off the Keychain until the user has granted it once.
fn refresh_stale_usage(app: &AppHandle, state: &Arc<AppState>) {
    if !state.may_fetch_usage() {
        return;
    }
    let age = now_ms() - state.usage().fetched_at;
    if age < STALE_AFTER_MS {
        return;
    }

    let app = app.clone();
    let state = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        crate::commands::fetch_usage(&app, &state).await;
    });
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
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
