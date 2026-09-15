//! Panel show and hide orchestration. Rust owns the timing: the webview never
//! decides when a window appears or disappears. tech.md section 8.

use std::sync::Arc;
use std::time::{Duration, Instant};

use peekle_core::island::{shape_rect, Rect};
use peekle_core::types::{
    IslandView, PromptKind, PromptOutcome, PromptRequest, ToastRequest, ToastTone,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::platform;
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
    let gate = app.state::<Arc<AppState>>().ready_gate(platform::ISLAND);
    let _ = tokio::time::timeout(READY_TIMEOUT, gate.notified()).await;

    on_main(app, "show", move |handle| {
        if let Err(err) = platform::show(handle, platform::ISLAND) {
            tracing::error!(error = %err, "failed to show the island");
        }
    });
}

/// Hands the webview the notch of the display the island is on. Cheap enough
/// to repeat on every open, and the only alternative is reloading the route,
/// which throws away the feed and whatever the user had typed. tech.md 6.7.
pub fn send_notch(app: &AppHandle) {
    let (height, width) = platform::active_notch(app).unwrap_or((0.0, 0.0));
    let payload = serde_json::json!({ "height": height, "width": width });

    if let Err(err) = app.emit_to(platform::ISLAND, events::NOTCH, payload) {
        tracing::warn!(error = %err, "failed to emit notch");
    }
}

/// How often Rust asks where the pointer is while the island rests.
///
/// The safety net, not the mechanism: `platform::watch_pointer` hands the mouse
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
    if let Err(err) = platform::watch_pointer(app, update_hover) {
        tracing::warn!(error = %err, "no mouse monitor, the mark falls back to the poll");
    }
    if let Err(err) = platform::watch_clicks(app, clicked_elsewhere) {
        tracing::warn!(error = %err, "no click monitor, a click elsewhere waits for the leave clock");
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

/// The rectangle the shape was drawn in, in physical pixels of the screen, or
/// None before the webview has measured one. Guessing one would eat clicks
/// next to a mark the user cannot even see. tech.md 6.7.
pub(crate) fn shape_on_screen(app: &AppHandle, state: &AppState) -> Option<Rect> {
    let bounds = state.shape_bounds()?;
    let (frame, scale) = platform::island_frame(app).ok()?;
    let mark = Rect::new(
        bounds.x * scale,
        bounds.y * scale,
        bounds.width * scale,
        bounds.height * scale,
    );
    shape_rect(frame, mark)
}

fn update_hover(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();

    let Some(rect) = shape_on_screen(app, &state) else {
        return;
    };
    let Ok(pointer) = app.cursor_position() else {
        return;
    };
    let inside = rect.contains((pointer.x, pointer.y));

    // The two states that draw something smaller than the window take the
    // mouse by the rectangle they drew and nowhere else, or a transparent 720
    // by 560 window eats every click on whatever is underneath. The pill
    // joined the mark in v80.21, when it got something to press: it opens the
    // chat it is about. tech.md 6.7 and 6.2.
    let view = state.view();
    if tracks_pointer(&view, state.shrunk()) {
        // A pill runs on its own clock either way. A request in flight does
        // not stop this: hiding the shape resolves nothing, the hook stays
        // pending, and the mark pulses until it is answered. tech.md 6.7.
        state.pointer_returned();
        if state.set_over_rest(inside) {
            if let Err(err) = platform::set_takes_clicks(app, inside) {
                tracing::error!(error = %err, "failed to switch cursor events for the shape");
            }
        }
        return;
    }

    // An open island already takes the mouse outright, and `set_view` said so.
    state.set_over_rest(false);

    // A question stands until it is answered. Every other open state is the
    // island showing something, and walking away from something you are shown
    // is a way of being done with it; a question is the one state where the
    // agent is parked waiting for this person, and taking it off the screen
    // while they think, or while they go and look at what it is about, leaves
    // them with a pulsing mark instead of the question they were reading.
    // Putting it away by hand still works -- a click outside is a decision,
    // not a wander -- and then the mark says who is waiting. tech.md 6.7
    // and 6.14.
    if stands_until_answered(state.active_prompt().as_ref()) {
        state.pointer_returned();
        return;
    }
    // A question the island asked stands the same way: ⌥⌘Q is pressed with the
    // pointer anywhere, and an update is offered while nobody is near the
    // shape at all. A panel that goes away before the hand reaches it asks
    // nothing. A click beside it is still an answer, and it is no.
    // tech.md 6.29 and 6.30.
    if question_stands(&view) {
        state.pointer_returned();
        return;
    }
    // An island in the user's hands is not one they are walking away from: a
    // picture open, the cursor in the field or a draft in it, a dialog up, the
    // CLI asking about the folder. The screenshot frame of ⌃⇧⌘4 takes the
    // pointer off the shape and the key window with it, and this is what used
    // to put the island away under a person in the middle of a sentence.
    // tech.md 6.7, 6.13, 6.23 and 6.24.
    if state.in_hand() {
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
    // Still where the shape left it: the island shrank, the hand did not walk
    // away, and there is nothing to charge to the leave clock. The pin is put
    // down by `island_bounds` as the shape settles, and only under a hand that
    // was on the shape. tech.md 6.7.
    if state.pointer_pinned((pointer.x, pointer.y), POINTER_MOVED) {
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

/// Whether the window gives its clicks back everywhere but the shape it drew,
/// rather than taking the whole 720 by 560 of itself.
///
/// The two small views do, and so does any view run down to the attachment
/// strip: the system file dialog stands directly under the island, and a
/// window that keeps every click is a dialog whose sidebar, search field and
/// path bar cannot be pressed. tech.md 6.7 and 6.25.
fn tracks_pointer(view: &IslandView, shrunk: bool) -> bool {
    matches!(view, IslandView::Collapsed | IslandView::Pill) || shrunk
}

/// The island shrank to the strip, or grew back out of it.
///
/// No view changed, so nothing else does either of the two things only Rust
/// can: hand the mouse back to whatever is underneath, and let go of the digit
/// that opens the island up again. tech.md 6.25.
pub fn apply_shrunk(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    let takes_clicks = state.view().takes_clicks() && !state.shrunk();
    on_main(app, "shrunk", move |handle| {
        if let Err(err) = platform::set_takes_clicks(handle, takes_clicks) {
            tracing::error!(error = %err, "failed to switch cursor events for the strip");
        }
    });
    sync_choice_keys(app);
}

/// Where the pointer stands, if it stands on the shape as the state knows it
/// now. Main thread only, like everything that asks AppKit. tech.md 6.7.
pub(crate) fn hand_on_shape(app: &AppHandle, state: &AppState) -> Option<(f64, f64)> {
    let rect = shape_on_screen(app, state)?;
    let pointer = app.cursor_position().ok()?;
    pin_for(rect, (pointer.x, pointer.y))
}

/// The point to pin, or none: a hand on the shape is pinned where it is, a
/// hand that had already left it is walking away and is not. tech.md 6.7.
fn pin_for(shape: Rect, pointer: (f64, f64)) -> Option<(f64, f64)> {
    shape.contains(pointer).then_some(pointer)
}

/// ⌥⌘Q: the island opens just enough to ask whether to quit. Pressed again
/// while it asks, it changes nothing: `set_view` ignores a view that stands.
/// tech.md 6.29.
pub fn ask_quit(app: &AppHandle) {
    set_view(app, IslandView::Quit);
}

/// Whether the leave clock leaves this view alone. tech.md 6.29 and 6.30.
fn question_stands(view: &IslandView) -> bool {
    matches!(
        view,
        IslandView::Quit | IslandView::Update | IslandView::Bug
    )
}

/// A mouse button went down in another application.
fn clicked_elsewhere(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    let view = state.view();

    // A press on the resting mark that the window did not take. The mark
    // takes the mouse only once the hover tracker has reacted to the pointer
    // arriving, and that reaction is a main-thread event some tens of
    // milliseconds behind the pointer -- more on a loaded Mac. A hand that
    // arrives and presses inside that window sends its click through to the
    // menu bar, and this monitor is where such a click ends up. Measured
    // live: a press 0 ms after arrival was lost every time, 20 ms after
    // arrival never. The click is spent on what it was aimed at. tech.md 6.7.
    if view == IslandView::Collapsed {
        let inside = shape_on_screen(app, &state)
            .zip(app.cursor_position().ok())
            .is_some_and(|(rect, pointer)| rect.contains((pointer.x, pointer.y)));
        if press_on_rest_opens(&view, inside) {
            tracing::debug!("a press reached the mark before the window took the mouse, opening");
            set_view(app, IslandView::Sessions);
        }
        return;
    }

    if click_elsewhere_collapses(&view, state.in_hand(), state.held_open(Instant::now())) {
        tracing::debug!("a click landed in another application, putting the island away");
        set_view(app, IslandView::Collapsed);
    }
}

/// Whether a press the window did not take opens the island: only on a
/// resting island, and only inside the mark it drew. A pill is not the mark,
/// and a press beside the mark is a press on the menu bar. tech.md 6.7.
fn press_on_rest_opens(view: &IslandView, inside: bool) -> bool {
    matches!(view, IslandView::Collapsed) && inside
}

/// Whether a press in another application puts the island away at once.
///
/// A click beside the shape inside the window already does, and a click past
/// the window is the same decision. Everything that keeps the leave clock from
/// running keeps this from firing too: the island in the user's hands -- the
/// frame of ⌃⇧⌘4 is a press in another process, and it must not fold a chat
/// being written in -- and an island that opened by itself and that nobody
/// came to, which a click in the editor says nothing about. A pill and a
/// resting mark are not open. tech.md 6.7.
fn click_elsewhere_collapses(view: &IslandView, in_hand: bool, held: bool) -> bool {
    view.takes_clicks() && !in_hand && !held
}

/// The one way the island changes shape. Stores the intent, tells the webview
/// to redraw, and switches mouse handling to match. A view that has not moved
/// does nothing at all.
pub fn set_view(app: &AppHandle, view: IslandView) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    if !state.set_view(view.clone()) {
        return;
    }

    if let Err(err) = app.emit_to(platform::ISLAND, events::VIEW, &view) {
        tracing::warn!(error = %err, "failed to emit view");
    }

    // A view that stands as the strip keeps the strip's mouse: the dialog it
    // shrank for is still underneath. tech.md 6.25.
    let takes_clicks = view.takes_clicks() && !state.shrunk();
    let opening = !matches!(view, IslandView::Collapsed);

    // An island that opens on a stale snapshot draws it and refreshes behind
    // itself. It never waits: a slow network must not delay the shape.
    // tech.md 6.4.
    if opening {
        refresh_stale_usage(app, &state);
    }

    let focus_view = view.clone();
    on_main(app, "view", move |handle| {
        if let Err(err) = platform::set_takes_clicks(handle, takes_clicks) {
            tracing::error!(error = %err, "failed to switch cursor events");
        }
        // Whether the window may take focus follows the view on the platforms
        // where that is not the panel's own call. tech.md 6.27.
        platform::apply_view(handle, &focus_view);

        // Re-order on every open. Section 8 forbids hiding the panel between
        // events, not re-asserting its order: the window server decides space
        // membership when a window is ordered front, so a panel ordered once at
        // startup never follows the user onto a full screen space.
        if opening {
            if let Err(err) = platform::show(handle, platform::ISLAND) {
                tracing::error!(error = %err, "failed to raise the island");
            }
            // The island may have just landed on a different display, and the
            // shape has to grow out of that display's bezel. tech.md 6.7.
            send_notch(handle);
        }
    });

    // A question that opened or went away with the view takes its keys with
    // it. tech.md 6.14.
    sync_choice_keys(app);
}

/// How many of ⌘1…⌘9 a question on screen needs: every row of its widest
/// question, the row for an answer of one's own included. None while no
/// question stands, or while the island is not open on the chat it is about --
/// a resting island is nobody's to answer by keyboard, and taking ⌘1 from the
/// browser for a question nobody can see would be theft. tech.md 6.14.
pub fn choice_keys_for(view: &IslandView, active: Option<&PromptRequest>) -> u8 {
    let Some(prompt) = active else {
        return 0;
    };
    let on_its_chat = matches!(view, IslandView::Session(id) if *id == prompt.session.session_id);
    match prompt.kind {
        // Deny and Allow, the two buttons in the order they stand: ⌘1 the
        // left, ⌘2 the right. On the compact panel and on the chat alike --
        // the owner chose these over rarer combinations, knowing ⌘2 in a
        // browser while a request stands allows it. tech.md 6.7, v87.8.
        PromptKind::Permission => {
            return if matches!(view, IslandView::Ask) || on_its_chat {
                2
            } else {
                0
            };
        }
        PromptKind::Question => {}
        _ => return 0,
    }
    if !on_its_chat {
        return 0;
    }
    let widest = prompt
        .questions
        .iter()
        .map(|question| question.options.len() + 1)
        .max()
        .unwrap_or(0);
    widest.min(9) as u8
}

/// How many of ⌘1…⌘9 Rust holds right now, for whatever reason it holds them.
///
/// A standing question asks first and is never overruled: the agent is parked
/// on it, and an island shrunk to the strip is waiting for nobody. Only when
/// the question asks for none does the strip get ⌘1, and only to open the
/// island back up. One digit can mean one thing at a time. tech.md 6.14, 6.25.
pub fn digit_keys_for(view: &IslandView, active: Option<&PromptRequest>, shrunk: bool) -> u8 {
    let asked = choice_keys_for(view, active);
    if asked > 0 {
        return asked;
    }
    u8::from(shrunk)
}

/// The config spelling of ⌘ and one digit.
pub fn choice_spelling(index: u8) -> String {
    format!("Command+Digit{index}")
}

/// Takes the choice keys the question on screen needs and gives back the rest.
///
/// Off the calling thread, always: the plugin registers on the main thread
/// under its registry lock, and this is reached from `set_view`, which the
/// shortcut handler itself calls (⌥⌘Q). Reads the view and the prompt when it
/// runs rather than when it was asked, so two calls in a row settle on what is
/// true now, and the lock makes them take turns. tech.md 6.14 and 6.13.
pub fn sync_choice_keys(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<Arc<AppState>>().inner().clone();
        let mut held = state.choice_keys();
        let want = digit_keys_for(
            &state.view(),
            state.active_prompt().as_ref(),
            state.shrunk(),
        );
        if *held == want {
            return;
        }
        for index in (want + 1)..=*held {
            crate::hotkey::unregister(&handle, &choice_spelling(index));
        }
        for index in (*held + 1)..=want {
            if let Err(err) = crate::hotkey::register(&handle, &choice_spelling(index)) {
                // Another app holds it. The row still answers to a click, and
                // to ⌘ and the digit inside the island. tech.md 6.14.
                tracing::debug!(index, error = %err, "a choice key is taken");
            }
        }
        tracing::debug!(held = want, "choice keys");
        *held = want;
    });
}

/// ⌘ and a digit fired: the island picks that row of the question on screen.
/// Only an event -- nothing here touches the plugin, so it is safe from inside
/// the shortcut handler. tech.md 6.14.
pub fn choose(app: &AppHandle, index: u8) {
    let payload = serde_json::json!({ "index": index });
    if let Err(err) = app.emit_to(platform::ISLAND, events::CHOOSE, payload) {
        tracing::warn!(error = %err, "failed to emit a choice");
    }
}

/// A second launch reached this instance: open the list and hold it the way
/// a notice is held. Nobody's pointer is on an island that opened by itself,
/// and without the hold the leave rule of 6.7 would put it away 800ms later,
/// which is a flash, not an answer. tech.md 6.7.
pub fn show_for_a_second_launch(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    state.hold_open(Instant::now() + NOTICE_HOLD);
    set_view(app, IslandView::Sessions);
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
    let gate = state.ready_gate(platform::ISLAND);

    if let Err(err) = app.emit_to(platform::ISLAND, events::PROMPT_OPEN, request) {
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
    // The view may not have changed -- the island was already on this chat --
    // and the question still needs its keys. tech.md 6.14.
    sync_choice_keys(app);
}

/// Whether what is on screen has to stay there until it is answered.
///
/// Every other open state is the island showing something, and walking away
/// from something you are shown is a way of being done with it. A question is
/// the one state where the agent is parked waiting for this person: taking it
/// off the screen while they think, or while they go and look at the thing it
/// is about, leaves them with a pulsing mark instead of the question they were
/// reading. Putting it away by hand still works -- a click beside the shape is
/// a decision, not a wander -- and then the mark says who is waiting.
/// tech.md 6.7 and 6.14.
fn stands_until_answered(active: Option<&PromptRequest>) -> bool {
    active.is_some_and(|request| request.kind == PromptKind::Question)
}

/// Whether a pill has to stay down because something is waiting on the person.
///
/// Every request, not only a question: a pill replaces the view outright, so
/// raising one over an open request throws away what the person is reading
/// and, three seconds later, the island with it. What the pill had to say is
/// dropped rather than queued -- by the time the request is answered it is
/// about a moment that has passed, and the notification it came from has
/// already rung as a system banner (6.17). tech.md 6.7.
fn pill_holds_off(active: Option<&PromptRequest>) -> bool {
    active.is_some()
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
///
/// Says whether it opened. An engaged island showing something else is left
/// with what it shows: the person is reading or writing there, and another
/// chat's answer put over it takes that away. The caller rings the banner
/// instead (6.17). The chat already on screen counts as shown. tech.md 6.2
/// and 6.7.
pub async fn reveal_turn(app: &AppHandle, session_id: &str) -> bool {
    let state = app.state::<Arc<AppState>>().inner().clone();

    if state.active_prompt().is_some() {
        tracing::debug!(session = %session_id, "a request already holds the view");
        return false;
    }
    if !reveal_takes(&state.view(), session_id, state.engaged(Instant::now())) {
        tracing::debug!(session = %session_id, "the island is engaged elsewhere, so no reveal");
        return false;
    }

    let gate = state.ready_gate(platform::ISLAND);
    let _ = tokio::time::timeout(READY_TIMEOUT, gate.notified()).await;
    set_view(app, IslandView::Session(session_id.to_string()));
    state.hold_open(Instant::now() + NOTICE_HOLD);
    true
}

/// Whether a turn ending in `session_id` may take the view. tech.md 6.2.
fn reveal_takes(current: &IslandView, session_id: &str, engaged: bool) -> bool {
    if matches!(current, IslandView::Session(open) if open == session_id) {
        return true;
    }
    !engaged
}

/// Tells the webview which outcome settled the request and collapses the
/// island behind it.
pub fn close_prompt(app: &AppHandle, prompt_id: &str, outcome: &PromptOutcome) {
    let payload = serde_json::json!({ "prompt_id": prompt_id, "outcome": outcome });
    if let Err(err) = app.emit_to(platform::ISLAND, events::PROMPT_CLOSE, payload) {
        tracing::warn!(error = %err, "failed to emit prompt-close");
    }

    // Long enough to read as a request settling, short enough not to be a wait.
    // Skipped when another prompt is already queued behind this one: collapsing
    // and reopening in the same breath reads as a glitch.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(COLLAPSE_AFTER).await;
        let state = handle.state::<Arc<AppState>>().inner().clone();
        // An answered question gives its keys back even where the island stays
        // open on the chat. tech.md 6.14.
        sync_choice_keys(&handle);
        if state.active_prompt().is_some() {
            return;
        }
        if collapses_after_answer(&state.view(), state.engaged(Instant::now())) {
            set_view(&handle, IslandView::Collapsed);
        }
    });
}

/// Whether a settled request takes the island down with it.
///
/// The compact panel existed for the answer and goes with it. A dialogue does
/// not: the person answered from the row inside the feed, which means they
/// came to it and are reading the chat the request stood in, and taking the
/// chat away is not what an answer means. An open dialogue nobody came to --
/// answered from the terminal, say, or timed out -- goes as it did. tech.md
/// 6.7.
fn collapses_after_answer(view: &IslandView, engaged: bool) -> bool {
    !(matches!(view, IslandView::Session(_)) && engaged)
}

/// One warning on startup when the combination could not be taken. Swallowing
/// it would leave the user pressing a key that does nothing. tech.md 6.9.
pub fn warn_hotkey(app: &AppHandle, text: &str) {
    toast(
        app,
        ToastRequest {
            session: None,
            text: text.to_string(),
            detail: None,
            took_ms: None,
            tone: ToastTone::Warn,
            ttl_ms: 3200,
            badge: None,
        },
    );
}

/// A toast is the `Pill` view for as long as it lives. Nothing here waits on
/// the user, so the island collapses itself when the time is up.
///
/// Unless something is waiting on them. `set_view` does not queue behind what
/// is on screen, it replaces it: a pill raised over a standing request took
/// the request off the screen and then collapsed the island on its own clock,
/// with the hook still pending and an answer half typed into the field. The
/// same rule the end of a turn (6.2) and a screenshot offer (6.13) already
/// keep, in the one place every pill goes through. tech.md 6.7.
pub fn toast(app: &AppHandle, request: ToastRequest) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    if pill_holds_off(state.active_prompt().as_ref()) {
        tracing::debug!("a request is standing, so the pill stays down");
        return;
    }
    // The same for an island somebody is using. A pill over a dialogue being
    // written in wiped the dialogue and, on its own ttl, put the island away
    // under the person. What it had to say is dropped, not queued. tech.md 6.7.
    if state.engaged(Instant::now()) {
        tracing::debug!("the island is engaged, so the pill stays down");
        return;
    }

    let ttl = Duration::from_millis(u64::from(request.ttl_ms));

    if let Err(err) = app.emit_to(platform::ISLAND, events::TOAST, &request) {
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
    use super::{
        choice_keys_for, choice_spelling, click_elsewhere_collapses, collapses_after_answer,
        digit_keys_for, pill_holds_off, reveal_takes, stands_until_answered, tracks_pointer,
        view_for, ASK_HOLD, DISMISS_AFTER, NOTICE_HOLD, PROMPT_HOLD,
    };
    use peekle_core::types::{
        IslandView, PromptKind, PromptRequest, Question, QuestionOption, SessionRef,
    };

    fn asking(options: &[usize]) -> PromptRequest {
        let mut request = standing(PromptKind::Question);
        request.questions = options
            .iter()
            .map(|count| Question {
                header: "h".into(),
                question: format!("{count} options?"),
                options: (0..*count)
                    .map(|index| QuestionOption {
                        label: format!("o{index}"),
                        description: None,
                    })
                    .collect(),
                multi_select: false,
            })
            .collect();
        request
    }

    /// v87.5: ⌘1…⌘N are held while a question stands open on its own chat, one
    /// per row of the widest question with the row for an answer of one's own
    /// counted, and not a single one otherwise. tech.md 6.14.
    #[test]
    fn choice_keys_are_held_only_while_a_question_stands_open_on_its_chat() {
        let on_it = IslandView::Session("s".into());
        assert_eq!(choice_keys_for(&on_it, Some(&asking(&[3]))), 4);
        assert_eq!(
            choice_keys_for(&on_it, Some(&asking(&[2, 4]))),
            5,
            "the widest question decides"
        );

        assert_eq!(choice_keys_for(&on_it, None), 0, "no question");
        assert_eq!(
            choice_keys_for(&on_it, Some(&standing(PromptKind::Idle))),
            0,
            "a notice answers nothing"
        );
        for view in [
            IslandView::Collapsed,
            IslandView::Pill,
            IslandView::Sessions,
            IslandView::Session("another".into()),
        ] {
            assert_eq!(
                choice_keys_for(&view, Some(&asking(&[3]))),
                0,
                "nobody can see the question on {view:?}"
            );
        }
        assert_eq!(
            choice_keys_for(&on_it, Some(&asking(&[12]))),
            9,
            "there are nine digits"
        );
    }

    /// v87.10: the strip takes ⌘1 to open the island back up, and only while
    /// no question is asking for the digits. One digit means one thing at a
    /// time, and the thing with an agent parked on it wins. tech.md 6.25.
    #[test]
    fn the_strip_takes_one_digit_and_never_the_question_s() {
        let on_it = IslandView::Session("s".into());

        assert_eq!(digit_keys_for(&on_it, None, true), 1, "the way back");
        assert_eq!(digit_keys_for(&on_it, None, false), 0, "nothing to open up");

        assert_eq!(
            digit_keys_for(&on_it, Some(&asking(&[3])), true),
            4,
            "the question keeps every digit it asked for"
        );
        assert_eq!(
            digit_keys_for(
                &IslandView::Ask,
                Some(&standing(PromptKind::Permission)),
                true
            ),
            2,
            "so does a permission"
        );
        // A question nobody can see holds nothing, and then the strip is free
        // to take its one digit.
        assert_eq!(
            digit_keys_for(&IslandView::Sessions, Some(&asking(&[3])), true),
            1
        );
    }

    /// v87.10: the strip hands the mouse back the way the mark and the pill do.
    /// The file dialog stands under the island, and a window that keeps every
    /// click is a dialog that cannot be pressed. tech.md 6.7 and 6.25.
    #[test]
    fn a_shrunk_island_gives_its_clicks_back_like_the_mark() {
        for view in [
            IslandView::Sessions,
            IslandView::Session("s".into()),
            IslandView::Ask,
        ] {
            assert!(!tracks_pointer(&view, false), "{view:?} takes the window");
            assert!(tracks_pointer(&view, true), "{view:?} shrunk to the strip");
        }
        assert!(tracks_pointer(&IslandView::Collapsed, false));
        assert!(tracks_pointer(&IslandView::Pill, false));
    }

    /// v87.8: a permission holds ⌘1 and ⌘2, Deny and Allow, while it stands on
    /// the compact panel or on its own chat, and nowhere else. tech.md 6.7.
    #[test]
    fn a_permission_holds_two_keys_while_it_stands_open() {
        let permission = standing(PromptKind::Permission);
        assert_eq!(choice_keys_for(&IslandView::Ask, Some(&permission)), 2);
        assert_eq!(
            choice_keys_for(&IslandView::Session("s".into()), Some(&permission)),
            2
        );
        for view in [
            IslandView::Collapsed,
            IslandView::Pill,
            IslandView::Sessions,
            IslandView::Session("another".into()),
            IslandView::Quit,
        ] {
            assert_eq!(
                choice_keys_for(&view, Some(&permission)),
                0,
                "nobody can see the request on {view:?}"
            );
        }
    }

    /// Every spelling a choice key is registered under names a real key, or the
    /// row it stands for could never be picked from outside. tech.md 6.14.
    #[test]
    fn every_choice_key_spells_a_real_combination() {
        for index in 1..=9 {
            let combination = peekle_hotkey::Combination::parse(&choice_spelling(index))
                .expect("a choice spelling parses");
            assert!(combination.command);
            assert!(
                crate::hotkey::shortcut_of(&combination).is_some(),
                "⌘{index} maps onto a key"
            );
        }
    }

    /// v87.1: the pin goes under a hand that is on the shape as it settles,
    /// and under nothing else. A pointer already outside is a person walking
    /// away, and pinning it there kept the island open over their browser
    /// until the mouse moved. tech.md 6.7.
    #[test]
    fn only_a_hand_on_the_shape_is_pinned_where_it_stands() {
        let shape = super::Rect::new(100.0, 0.0, 400.0, 300.0);
        assert_eq!(super::pin_for(shape, (250.0, 40.0)), Some((250.0, 40.0)));
        assert_eq!(super::pin_for(shape, (700.0, 900.0)), None);
        assert_eq!(
            super::pin_for(shape, (99.0, 40.0)),
            None,
            "just past the edge"
        );
    }

    /// v87.1: a press on the mark that arrived before the window took the
    /// mouse still opens the island; a press anywhere else does not, and a
    /// pill is not the mark. tech.md 6.7.
    #[test]
    fn a_press_the_window_missed_opens_the_island_only_on_the_mark() {
        assert!(super::press_on_rest_opens(&IslandView::Collapsed, true));
        assert!(!super::press_on_rest_opens(&IslandView::Collapsed, false));
        assert!(!super::press_on_rest_opens(&IslandView::Pill, true));
        assert!(!super::press_on_rest_opens(&IslandView::Sessions, true));
    }

    /// v85: the quit question stays until it is answered, and it takes the
    /// mouse like every open view. tech.md 6.29.
    #[test]
    fn the_quit_question_stands_until_answered() {
        assert!(super::question_stands(&IslandView::Quit));
        assert!(!super::question_stands(&IslandView::Sessions));
        assert!(IslandView::Quit.takes_clicks());
        assert!(click_elsewhere_collapses(&IslandView::Quit, false, false));
    }

    /// v87: the update question stands the same way. It is raised while nobody
    /// is near the shape, so a leave clock would take it away before the hand
    /// arrived; a click beside it is still an answer, and it is "later".
    /// tech.md 6.30.
    #[test]
    fn the_update_question_stands_until_answered() {
        assert!(super::question_stands(&IslandView::Update));
        assert!(IslandView::Update.takes_clicks());
        assert!(click_elsewhere_collapses(&IslandView::Update, false, false));
    }

    /// v87.4: the bug question stands the same way. The hand that pressed the
    /// button is already on its way to the answer, and a panel that goes away
    /// under it asks nothing; a click beside it is an answer, and it is
    /// "cancel". tech.md 6.22.
    #[test]
    fn the_bug_question_stands_until_answered() {
        assert!(super::question_stands(&IslandView::Bug));
        assert!(IslandView::Bug.takes_clicks());
        assert!(click_elsewhere_collapses(&IslandView::Bug, false, false));
        assert!(
            !crate::platform::focusable_for(&IslandView::Bug),
            "two buttons and no field: nothing to take the keyboard for"
        );
    }

    #[test]
    fn a_click_in_another_application_puts_an_open_island_away_at_once() {
        for view in [
            IslandView::Sessions,
            IslandView::Ask,
            IslandView::Session("s".into()),
        ] {
            assert!(click_elsewhere_collapses(&view, false, false), "{view:?}");
        }
    }

    #[test]
    fn a_click_elsewhere_leaves_an_island_in_hand_or_one_nobody_came_to() {
        let open = IslandView::Session("s".into());
        assert!(!click_elsewhere_collapses(&open, true, false));
        assert!(!click_elsewhere_collapses(&open, false, true));
    }

    #[test]
    fn a_click_elsewhere_has_nothing_to_put_away_on_a_mark_or_a_pill() {
        assert!(!click_elsewhere_collapses(
            &IslandView::Collapsed,
            false,
            false
        ));
        assert!(!click_elsewhere_collapses(&IslandView::Pill, false, false));
    }

    fn standing(kind: PromptKind) -> PromptRequest {
        PromptRequest {
            id: "p".into(),
            kind,
            session: SessionRef {
                session_id: "s".into(),
                cwd: "/tmp".into(),
                project: "tmp".into(),
                pid: None,
                tty: None,
            },
            title: "t".into(),
            tool: None,
            last_message: None,
            detail: None,
            options: Vec::new(),
            questions: Vec::new(),
            allow_free_text: false,
            created_at: 0,
            expires_at: 0,
        }
    }

    /// A question is the one thing on screen the agent is parked on. Walking
    /// away from it is not an answer, and it used to put the question away
    /// eight hundred milliseconds after the pointer left. tech.md 6.14.
    #[test]
    fn a_question_stays_until_it_is_answered() {
        assert!(stands_until_answered(Some(&standing(PromptKind::Question))));
    }

    /// Everything else keeps the rules it had. The compact panel runs its
    /// twenty seconds and the end of a turn is a notice nobody has to answer,
    /// so both may be walked away from. tech.md 6.7.
    #[test]
    fn nothing_else_holds_the_island_open() {
        for kind in [PromptKind::Permission, PromptKind::Idle] {
            assert!(!stands_until_answered(Some(&standing(kind))), "{kind:?}");
        }
        assert!(!stands_until_answered(None));
    }

    /// The panel is a question with two answers on it, so it does not need the
    /// forty five seconds a question with four options and descriptions does.
    /// What it must never be is shorter than the walk to it. tech.md 6.7.
    /// A pill takes the view outright, so raising one over a standing request
    /// takes the request off the screen and then collapses the island on the
    /// pill's own clock -- with the hook still pending and an answer half
    /// typed. Every kind, not only a question: the person is answering all of
    /// them. tech.md 6.7.
    #[test]
    fn a_pill_stays_down_while_anything_is_waiting_on_the_person() {
        for kind in [
            PromptKind::Question,
            PromptKind::Permission,
            PromptKind::Idle,
        ] {
            assert!(pill_holds_off(Some(&standing(kind))), "{kind:?}");
        }
    }

    /// And with nothing standing it is the ordinary pill it always was.
    #[test]
    fn a_pill_rises_when_nothing_is_waiting() {
        assert!(!pill_holds_off(None));
    }

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

    /// A turn ending in another chat used to take the view from a person
    /// reading or writing in this one. It may still take a view nobody has
    /// come to, and the chat already on screen is shown either way. tech.md
    /// 6.2 and 6.7.
    #[test]
    fn another_turn_does_not_take_an_engaged_island() {
        let reading = IslandView::Session("b".into());
        assert!(!reveal_takes(&reading, "a", true));
        assert!(!reveal_takes(&IslandView::Sessions, "a", true));
        assert!(reveal_takes(&reading, "a", false), "nobody came to it");
        assert!(
            reveal_takes(&reading, "b", true),
            "it is the chat on screen"
        );
        assert!(reveal_takes(&IslandView::Collapsed, "a", false));
    }

    /// The compact panel goes with its answer. The dialogue the person came to
    /// stays: they are reading it, and the row they answered from was in it.
    /// tech.md 6.7.
    #[test]
    fn an_answer_from_the_dialogue_keeps_the_dialogue() {
        assert!(collapses_after_answer(&IslandView::Ask, true));
        assert!(collapses_after_answer(&IslandView::Ask, false));
        assert!(!collapses_after_answer(
            &IslandView::Session("s".into()),
            true
        ));
        assert!(
            collapses_after_answer(&IslandView::Session("s".into()), false),
            "a dialogue nobody came to goes as it did"
        );
    }
}
