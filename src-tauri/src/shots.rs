//! The screenshot watch. tech.md 6.13.
//!
//! Control-Shift-Command-4 puts an image on the pasteboard and nowhere else,
//! and the channel to an agent carries text. So Peekle watches for the write,
//! offers to attach it, and on agreement turns the image into a file and the
//! file into a line of the next reply.
//!
//! AppKit lives here and in `panel.rs`, and nowhere else. tech.md 12.
//!
//! Two halves, deliberately unequal. Detection reads the change count and the
//! type names, which raises nothing and copies nothing, and it runs on a
//! timer. Reading the contents runs once, after the user pressed the key: from
//! macOS 15 that read is a system paste prompt, and asking for one on every
//! copy anybody makes would be a product that spies. tech.md R-13.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::shots::{self, Pasteboard};
use peekle_core::types::{IslandView, ShotOffer, ToastRequest, ToastTone};
use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::hotkey;
use crate::panel;
use crate::state::AppState;
use crate::windows;

/// The real pasteboard.
pub struct SystemPasteboard;

impl Pasteboard for SystemPasteboard {
    fn change_count(&self) -> i64 {
        use objc2_app_kit::NSPasteboard;

        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.changeCount() as i64
    }

    fn item_types(&self) -> Vec<Vec<String>> {
        use objc2_app_kit::NSPasteboard;

        let pasteboard = NSPasteboard::generalPasteboard();
        // The item and not the pasteboard: NSPasteboard synthesises TIFF from
        // a PNG, so its declared list calls every screenshot an image and
        // every image a screenshot. tech.md 6.13.
        let Some(items) = pasteboard.pasteboardItems() else {
            return Vec::new();
        };
        items
            .iter()
            .map(|item| item.types().iter().map(|t| t.to_string()).collect())
            .collect()
    }

    fn read_png(&self) -> Option<Vec<u8>> {
        use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG};

        let pasteboard = NSPasteboard::generalPasteboard();
        // The extern static is the type name itself, and reading one is what
        // `unsafe` covers here. Nothing about the read is fallible otherwise.
        let png = unsafe { NSPasteboardTypePNG };
        let data = pasteboard.dataForType(png)?;
        Some(data.to_vec())
    }
}

/// Starts the watch. Silent and cheap: one counter read per tick, and the
/// contents are never touched here. tech.md 6.13.
pub fn watch(app: &AppHandle, state: Arc<AppState>) {
    let (enabled, poll_ms, spelling) = {
        let config = state.lock_config();
        (
            config.shots.enabled,
            config.shots.poll_ms,
            config.hotkey.attach.clone(),
        )
    };
    if !enabled {
        tracing::debug!("the screenshot watch is switched off");
        return;
    }
    if spelling.is_empty() {
        // Nothing could answer an offer, and an offer nobody can answer is
        // just a line across the screen. tech.md 6.13.
        tracing::debug!("no attach key configured, so no screenshot offers");
        return;
    }

    // From what is on the pasteboard now, not from zero: the first tick must
    // not offer to attach whatever the user copied before Peekle started.
    state.seed_change_count(state.pasteboard.change_count());

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(poll_ms));
        loop {
            ticker.tick().await;
            tick(&handle, &state);
        }
    });
}

fn tick(app: &AppHandle, state: &Arc<AppState>) {
    let now = now_ms();

    // An offer nobody answered settles itself, and the Up arrow goes back to
    // whatever application the user is actually looking at. tech.md 6.13.
    if let Some(offer) = state.shot.take_expired(now) {
        tracing::debug!(id = %offer.id, "nobody answered the screenshot offer");
        close_offer(app, state);
        put_away(app, state);
    }

    let count = state.pasteboard.change_count();
    if !state.pasteboard_changed(count) {
        return;
    }
    if !shots::is_screenshot(&state.pasteboard.item_types()) {
        return;
    }
    open_offer(app, state, now);
}

/// Puts the offer up, or decides there is nothing to offer.
fn open_offer(app: &AppHandle, state: &Arc<AppState>, now: i64) {
    // An active permission request already holds the view and is the
    // stronger claim, the same way it outranks a `Stop` reveal. tech.md 6.7.
    // Forcing the pill over it would not queue behind it: `set_view` simply
    // overwrites the view, so the permission's own `Session` view is gone and
    // nothing ever brings it back once the pill's five seconds are up --
    // the pending hook is still blocking, but there is no longer any UI left
    // that can answer it. That is the freeze this guards against.
    if state.active_prompt().is_some() {
        tracing::debug!("a permission request holds the view, so no screenshot offer");
        return;
    }

    let Some(card) = state.newest_owned_session() else {
        // A screenshot with nowhere to go. An observed session has no input
        // field, so there is nothing to attach it to. tech.md 6.5 and 6.13.
        tracing::debug!("a screenshot arrived with no session of ours to take it");
        return;
    };

    let (secs, spelling) = {
        let config = state.lock_config();
        (config.shots.offer_secs, config.hotkey.attach.clone())
    };

    let offer = ShotOffer {
        id: shots::new_id(),
        session_id: card.session.session_id.clone(),
        project: card.session.project.clone(),
        created_at: now,
        expires_at: now + i64::from(secs) * 1000,
    };

    // The fresh screenshot pushes the stale one out: one key cannot answer for
    // two, and the one the user wants is the one they just took.
    if let Some(replaced) = state.shot.open(offer.clone()) {
        tracing::debug!(id = %replaced.id, "a newer screenshot replaced the offer");
    }

    if let Err(err) = hold_attach_key(app, state, &spelling) {
        // Nothing could answer it, so nothing is shown. Better than a line on
        // screen naming a key that does nothing. tech.md 6.13 and R-14.
        tracing::warn!(error = %err, "the attach key is unavailable, so no offer");
        state.shot.take();
        return;
    }

    tracing::debug!(session = %offer.session_id, "offering to attach a screenshot");
    emit_offer(app, Some(&offer));
    // The offer is one line of status, which is what `Pill` is. It runs on its
    // own clock exactly like a toast does. tech.md 6.7.
    windows::set_view(app, IslandView::Pill);
}

/// The user agreed. The one path that reads the contents of the pasteboard.
pub fn attach(app: &AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();

    // Exactly once. Everything below hangs off this take, so an offer that
    // expired a moment ago writes no file and opens no window. tech.md 6.13.
    let Some(offer) = state.shot.take() else {
        tracing::debug!("the attach key fired with no offer standing");
        return;
    };
    close_offer(app, &state);

    let Some(png) = state.pasteboard.read_png() else {
        // Every path from here down leaves the island where the offer left it,
        // so the pill has to come down on its own.
        // The user copied something else between the offer and the answer.
        tracing::warn!("the screenshot left the pasteboard before it was attached");
        say(app, "The screenshot is gone");
        return;
    };

    let Some(dir) = shots::shots_dir() else {
        tracing::warn!("no cache directory for screenshots");
        say(app, "Nowhere to save the screenshot");
        return;
    };
    let keep = state.lock_config().shots.keep;

    let path = match shots::write_shot(&dir, &offer.id, &png, keep) {
        Ok(path) => path.to_string_lossy().to_string(),
        Err(err) => {
            tracing::warn!(error = %err, "could not save the screenshot");
            say(app, "Could not save the screenshot");
            return;
        }
    };

    tracing::info!(session = %offer.session_id, bytes = png.len(), "attached a screenshot");
    let payload = serde_json::json!({ "session_id": offer.session_id, "path": path });
    if let Err(err) = app.emit_to(panel::ISLAND, events::SHOT_ATTACHED, payload) {
        tracing::warn!(error = %err, "failed to emit shot-attached");
    }

    // Attaching is not sending, so the island opens on the session with the
    // shot waiting in the field and the user writes what they want said.
    // tech.md 6.13.
    let handle = app.clone();
    let session_id = offer.session_id.clone();
    tauri::async_runtime::spawn(async move {
        windows::reveal_turn(&handle, &session_id).await;
    });
}

/// Drops the key and tells the island the offer is over. Runs on every path
/// that settles one, because a leaked offer holds the Up arrow away from the
/// whole system. tech.md 6.13 and R-14.
fn close_offer(app: &AppHandle, state: &Arc<AppState>) {
    release_attach_key(app, state);
    emit_offer(app, None);
}

/// Takes the pill down, and only the pill. Something more important may have
/// opened while the offer stood, and collapsing then would throw away a
/// session the user is reading. The agreement path skips this entirely: it
/// opens the session next, and a collapse in between reads as a glitch.
fn put_away(app: &AppHandle, state: &Arc<AppState>) {
    if state.view() == IslandView::Pill {
        windows::set_view(app, IslandView::Collapsed);
    }
}

fn emit_offer(app: &AppHandle, offer: Option<&ShotOffer>) {
    let payload = serde_json::json!({ "offer": offer });
    if let Err(err) = app.emit_to(panel::ISLAND, events::SHOT, payload) {
        tracing::warn!(error = %err, "failed to emit the screenshot offer");
    }
}

fn hold_attach_key(
    app: &AppHandle,
    state: &Arc<AppState>,
    spelling: &str,
) -> Result<(), peekle_hotkey::HotkeyError> {
    if !state.set_attach_key(true) {
        // Already held, from an offer this one replaced.
        return Ok(());
    }
    match hotkey::register(app, spelling) {
        Ok(()) => Ok(()),
        Err(err) => {
            state.set_attach_key(false);
            Err(err)
        }
    }
}

fn release_attach_key(app: &AppHandle, state: &Arc<AppState>) {
    if !state.set_attach_key(false) {
        return;
    }
    let spelling = state.lock_config().hotkey.attach.clone();
    hotkey::unregister(app, &spelling);
}

/// One line in the notch. Used only where the user pressed a key and got
/// nothing: silence there reads as a broken key rather than a missing shot.
fn say(app: &AppHandle, text: &str) {
    windows::toast(
        app,
        ToastRequest {
            text: text.to_string(),
            tone: ToastTone::Warn,
            ttl_ms: 1600,
            badge: None,
        },
    );
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}
