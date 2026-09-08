//! Panel show and hide orchestration. Rust owns the timing: the webview never
//! decides when a window appears or disappears. tech.md section 8.

use std::sync::Arc;
use std::time::{Duration, Instant};

use peekle_core::island::shape_rect;
use peekle_core::types::{
    IslandView, PromptKind, PromptOutcome, PromptRequest, ToastRequest, ToastTone,
};
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
/// The safety net, not the mechanism: `panel::watch_pointer` hands the mouse
/// over the moment the pointer moves onto the mark, because a poll is always
/// one tick behind and a click inside that tick went through to the menu bar.
/// What is left for the tick is the paths with no movement at all -- a Space
/// switch, a warped cursor -- and taking the mouse back on the way out, where
/// a hundred milliseconds is invisible. tech.md 6.7.
const HOVER_TICK: Duration = Duration::from_millis(100);

/// Hands the mouse to the island while the pointer is over the resting mark
/// and takes it back the moment it leaves.
///
/// A collapsed island is a transparent 720 by 560 rectangle. Letting it keep
/// the mouse would swallow every click in the top third of the screen, and
/// letting it never take the mouse would make the mark impossible to press.
/// tech.md 6.7.
pub fn track_pointer(app: &AppHandle) {
    // Movement leads. Installed on the main thread, where the setup runs, and
    // called back there too. tech.md 6.7.
    if let Err(err) = panel::watch_pointer(app, update_hover) {
        tracing::warn!(error = %err, "no mouse monitor, the mark falls back to the poll");
    }

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

/// How far the pointer travels before it counts as having moved at all.
///
/// Physical pixels, and small: a hand resting on a mouse jitters, and jitter
/// is not a decision to walk away from the island. tech.md 6.7.
const POINTER_MOVED: f64 = 10.0;

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

    // A pill runs on its own clock. A request in flight does not stop this:
    // hiding the shape resolves nothing, the hook stays pending, and the mark
    // pulses until it is answered. tech.md 6.7.
    if state.view() == IslandView::Pill {
        state.pointer_returned();
        return;
    }

    if inside {
        // Engaged, so the opening hold has done its job and ordinary leave
        // rules take over. tech.md 6.7.
        state.clear_hold();
        state.pointer_returned();
        return;
    }
    // The shape moved under the pointer, so the point it stands on is what
    // "did not move" means from here until it does. tech.md 6.7.
    if state.take_shape_moved() {
        state.anchor_pointer((pointer.x, pointer.y));
    }
    // Still on that point: the island shrank, the hand did not walk away, and
    // there is nothing to charge to the leave clock. tech.md 6.7.
    if state.pointer_pinned((pointer.x, pointer.y), POINTER_MOVED) {
        state.pointer_returned();
        return;
    }
    // A screenshot open at full size holds the island. The user opened the
    // picture by hand and closes it by hand, and reaching for the click that
    // closes it takes the pointer off the shape first: without this the island
    // was gone before the click landed. tech.md 6.13.
    if state.preview_open() {
        state.pointer_returned();
        return;
    }
    // Opened on its own and the hold is not up: it stays, and the leave clock
    // stays fresh so expiry gives the usual grace, not a snap.
    if state.held_open(Instant::now()) {
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
    let state = app.state::<Arc<AppState>>().inner().clone();
    let gate = state.ready_gate(panel::ISLAND);

    if let Err(err) = app.emit_to(panel::ISLAND, events::PROMPT_OPEN, request) {
        tracing::warn!(error = %err, "failed to emit prompt-open");
    }

    let _ = tokio::time::timeout(READY_TIMEOUT, gate.notified()).await;

    let next = view_for(request.kind, &request.session.session_id, &state.view());
    if let Some(view) = next.clone() {
        set_view(app, view);
    }

    // Shown, and now it waits, because a hook is pending and this form is the
    // only place it gets answered. Quiet all the way through means the user is
    // not there, and the island puts itself away with the request still
    // pending -- the panel's twenty seconds are the panel's, never the hook's.
    // tech.md 6.7.
    let hold = if next == Some(IslandView::Ask) {
        ASK_HOLD
    } else {
        PROMPT_HOLD
    };
    state.hold_open(Instant::now() + hold);
}

/// Which view a blocking request raises, or None to leave the island alone.
///
/// A permission asks for yes or no, and neither answer needs the feed, so it
/// gets the compact panel. Everything else -- a question with its own options,
/// a prompt at the end of a turn -- opens the session it belongs to. Nothing
/// moves at all when the island already stands on that very session: the
/// person is reading the thing the request is about, and swapping it for a
/// panel takes it away from them. tech.md 6.7.
fn view_for(kind: PromptKind, session_id: &str, current: &IslandView) -> Option<IslandView> {
    if matches!(current, IslandView::Session(open) if open == session_id) {
        return None;
    }
    Some(match kind {
        PromptKind::Permission => IslandView::Ask,
        _ => IslandView::Session(session_id.to_string()),
    })
}

/// How long an island opened by a blocking request waits for the user before
/// putting itself away.
///
/// Long enough to read the request and reach it: an `AskUserQuestion` carries
/// its own question, up to four options and a description under each, and ten
/// seconds ran out while the user was still reading. tech.md 6.7.
const PROMPT_HOLD: Duration = Duration::from_secs(45);

/// How long the compact permission panel stands before the island puts itself
/// away.
///
/// Short because there is nothing to read: a tool name, one line of input and
/// two buttons. The person who wants more presses the panel and lands in the
/// session, where the usual rules take over. Nothing is resolved when it runs
/// out -- the request stays pending and the mark goes on pulsing. tech.md 6.7.
const ASK_HOLD: Duration = Duration::from_secs(20);

/// The same for an island opened by a turn that just ended. Shorter on purpose:
/// nothing is pending, so an unread notice is not a reason to sit on top of the
/// screen. tech.md 6.7.
const NOTICE_HOLD: Duration = Duration::from_secs(10);

/// Opens the island on a session whose turn just ended. tech.md 6.2.
///
/// The promise of section 1: the agent finished, the notch opens and shows
/// what came of it. Since v41 the island is the chat, so the reply arrives
/// here and the user waits for it where they typed -- an island that takes the
/// text and never shows the answer has to be opened by hand.
///
/// Passive in exactly the way a request is: the panel takes the mouse and
/// never the keyboard, and a pointer that never arrives puts it away again.
/// An open request outranks this and keeps the view it already has.
pub async fn reveal_turn(app: &AppHandle, session_id: &str) {
    let state = app.state::<Arc<AppState>>().inner().clone();

    if state.active_prompt().is_some() {
        tracing::debug!(session = %session_id, "a request already holds the view");
        return;
    }

    let gate = state.ready_gate(panel::ISLAND);
    let _ = tokio::time::timeout(READY_TIMEOUT, gate.notified()).await;
    set_view(app, IslandView::Session(session_id.to_string()));
    state.hold_open(Instant::now() + NOTICE_HOLD);
}

/// Tells the webview which outcome settled the request and collapses the
/// island behind it.
pub fn close_prompt(app: &AppHandle, prompt_id: &str, outcome: &PromptOutcome) {
    let payload = serde_json::json!({ "prompt_id": prompt_id, "outcome": outcome });
    if let Err(err) = app.emit_to(panel::ISLAND, events::PROMPT_CLOSE, payload) {
        tracing::warn!(error = %err, "failed to emit prompt-close");
    }

    // Long enough to read as a request settling, short enough not to be a wait.
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

#[cfg(test)]
mod tests {
    use super::{view_for, ASK_HOLD, DISMISS_AFTER, NOTICE_HOLD, PROMPT_HOLD};
    use peekle_core::types::{IslandView, PromptKind};

    /// The panel is a question with two answers on it, so it does not need the
    /// forty five seconds a question with four options and descriptions does.
    /// What it must never be is shorter than the walk to it. tech.md 6.7.
    #[test]
    fn the_permission_panel_stands_for_twenty_seconds() {
        assert_eq!(ASK_HOLD, Duration::from_secs(20));
        assert!(ASK_HOLD < PROMPT_HOLD);
        assert!(ASK_HOLD > DISMISS_AFTER);
    }

    #[test]
    fn a_permission_raises_the_panel_and_everything_else_raises_the_session() {
        let collapsed = IslandView::Collapsed;
        assert_eq!(
            view_for(PromptKind::Permission, "s1", &collapsed),
            Some(IslandView::Ask)
        );
        assert_eq!(
            view_for(PromptKind::Question, "s1", &collapsed),
            Some(IslandView::Session("s1".into()))
        );
        assert_eq!(
            view_for(PromptKind::Idle, "s1", &collapsed),
            Some(IslandView::Session("s1".into()))
        );
    }

    /// Reading the feed of the session that is asking is the best place to be
    /// asked from. Nothing takes that away.
    #[test]
    fn a_session_already_on_screen_is_left_alone() {
        let open = IslandView::Session("s1".into());
        assert_eq!(view_for(PromptKind::Permission, "s1", &open), None);
        assert_eq!(
            view_for(PromptKind::Permission, "s2", &open),
            Some(IslandView::Ask),
            "another session asking is still news"
        );
    }

    use std::time::Duration;

    /// The v44.1 rule: what waits on the user stays up longer than what only
    /// reports. Equal holds are the bug this split fixed — a question was gone
    /// before it could be read. tech.md 6.7.
    #[test]
    fn a_request_is_held_longer_than_a_turn_notice() {
        assert!(PROMPT_HOLD > NOTICE_HOLD);
        assert!(
            PROMPT_HOLD >= Duration::from_secs(45),
            "reading a question with four described options takes longer than a glance"
        );
        assert_eq!(NOTICE_HOLD, Duration::from_secs(10));
    }

    /// The hold is the opening grace, the leave clock runs after it. A hold
    /// shorter than the leave clock would invert the two and put the island
    /// away while the pointer was still on its way.
    #[test]
    fn the_hold_outlasts_the_leave_clock() {
        assert!(NOTICE_HOLD > DISMISS_AFTER);
    }
}
