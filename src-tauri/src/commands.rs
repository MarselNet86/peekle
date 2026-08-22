//! Tauri commands. Names and payloads are the literal table of tech.md 6.5.
//! The frontend calls nothing else.

use std::sync::Arc;

use peekle_core::types::{
    Delivery, IslandView, PeekleState, PromptAnswer, PromptOutcome, ToastRequest, ToastTone,
    UsageSnapshot,
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events;
use crate::state::AppState;
use crate::windows;

#[tauri::command]
pub fn get_state(state: State<'_, Arc<AppState>>) -> PeekleState {
    state.snapshot()
}

/// Resolves the pending hook and hides the panel. Answering an id that is
/// already settled is a no-op, not an error. tech.md 6.5.
#[tauri::command]
pub fn answer_prompt(app: AppHandle, state: State<'_, Arc<AppState>>, answer: PromptAnswer) {
    let prompt_id = answer.prompt_id.clone();
    let outcome = PromptOutcome::Answered(answer);
    settle(&app, &state, &prompt_id, outcome);
}

#[tauri::command]
pub fn dismiss_prompt(app: AppHandle, state: State<'_, Arc<AppState>>, prompt_id: String) {
    settle(&app, &state, &prompt_id, PromptOutcome::Dismissed);
}

fn settle(app: &AppHandle, state: &Arc<AppState>, prompt_id: &str, outcome: PromptOutcome) {
    if !state.pending.resolve(prompt_id, outcome.clone()) {
        // Already settled by a timeout, a bypass or an earlier answer.
        return;
    }

    if let Some(request) = state.active_prompt() {
        if request.id == prompt_id {
            let at = now_ms();

            // An answer is a message the user sent, so it lands in the feed the
            // way a message does. A reply that vanishes on submit reads as one
            // that never went. tech.md 6.5.
            let answered = match &outcome {
                PromptOutcome::Answered(answer) => answer.text.as_deref(),
                _ => None,
            };
            if let Some(text) = answered {
                state.user_turn(
                    &request.session,
                    text,
                    peekle_core::types::EntryState::Ok,
                    at,
                );
            }

            // Answering unblocks the hook, so the agent is running again by the
            // time this returns. Saying idle would leave the island still while
            // work is happening. tech.md 6.3.
            let status = if answered.is_some() {
                peekle_core::types::SessionStatus::Working
            } else {
                peekle_core::types::SessionStatus::Idle
            };
            let cards = state.set_session_status(&request.session, status, at);
            if let Err(err) = app.emit(events::SESSIONS, &cards) {
                tracing::warn!(error = %err, "failed to emit sessions");
            }
        }
    }

    if let Some(next) = state.release_prompt(prompt_id) {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            windows::open_prompt(&handle, &next).await;
        });
    }

    // After release_prompt, so the collapse can tell whether anything queued
    // behind this one.
    windows::close_prompt(app, prompt_id, &outcome);
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// The hotkey path into the bypass switch. Reads the current value and flips
/// it, so the key means the same thing whichever way the switch is sitting.
pub fn toggle_enabled(app: AppHandle) {
    let state = app.state::<Arc<AppState>>().inner().clone();
    let next = !state.enabled();
    apply_enabled(&app, &state, next);
}

/// Bypass. Off resolves everything pending so no agent is left waiting.
#[tauri::command]
pub fn set_enabled(app: AppHandle, state: State<'_, Arc<AppState>>, enabled: bool) {
    let state = state.inner().clone();
    apply_enabled(&app, &state, enabled);
}

fn apply_enabled(app: &AppHandle, state: &Arc<AppState>, enabled: bool) {
    state.set_enabled(enabled);
    state.lock_config().behavior.enabled = enabled;

    if !enabled {
        state.pending.resolve_all(PromptOutcome::Bypassed);
    }

    if let Err(err) = app.emit(events::ENABLED, serde_json::json!({ "enabled": enabled })) {
        tracing::warn!(error = %err, "failed to emit enabled");
    }

    windows::toast(
        app,
        ToastRequest {
            text: if enabled {
                "Peekle is ON".to_string()
            } else {
                "Peekle is OFF".to_string()
            },
            tone: if enabled {
                ToastTone::On
            } else {
                ToastTone::Off
            },
            ttl_ms: 1600,
            badge: None,
        },
    );
}

/// Refreshes from whatever provider is configured. Runs off the async runtime
/// because the account provider blocks on a network call, and usage must never
/// hold up answering a hook. tech.md 6.4.
#[tauri::command]
pub async fn refresh_usage(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<UsageSnapshot, ()> {
    let state = state.inner().clone();
    Ok(fetch_usage(&app, &state).await)
}

pub async fn fetch_usage(app: &AppHandle, state: &Arc<AppState>) -> UsageSnapshot {
    let provider = Arc::clone(&state.usage_provider);
    let snapshot = match tauri::async_runtime::spawn_blocking(move || provider.snapshot()).await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            tracing::warn!(error = %err, "the usage provider panicked");
            return state.usage();
        }
    };

    // The outcome, not the token. Without this line a failing account leaves
    // nothing behind but `could not reach the API` on the bars.
    tracing::debug!(
        source = ?snapshot.source,
        reason = ?snapshot.reason,
        windows = snapshot.windows.len(),
        "usage snapshot"
    );
    state.set_usage(snapshot.clone());
    if let Err(err) = app.emit(events::USAGE, &snapshot) {
        tracing::warn!(error = %err, "failed to emit usage");
    }
    snapshot
}

/// The only path allowed to raise the Keychain dialog. Never called from
/// startup and never from the background poll. tech.md 6.4 and rule 12.
///
/// A grant is remembered so the poll may start; a refusal is remembered so
/// nothing asks again until the user clears it by hand.
#[tauri::command]
pub async fn request_usage_access(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<UsageSnapshot, ()> {
    let state = state.inner().clone();
    let snapshot = fetch_usage(&app, &state).await;

    {
        use peekle_core::types::UsageUnavailable;

        let mut config = state.lock_config();
        match snapshot.reason {
            Some(UsageUnavailable::Denied) => config.usage.keychain_denied = true,
            // A network failure says nothing about the Keychain: the read may
            // never have happened. Recording a grant on it would start the
            // background poll on a permission nobody confirmed.
            Some(UsageUnavailable::Network) => {}
            // Everything else means the read itself went through, entry there
            // or not, so the dialog will not come back.
            _ => {
                config.usage.keychain_denied = false;
                config.usage.keychain_granted = true;
            }
        }
    }
    state.save_config();

    Ok(snapshot)
}

#[tauri::command]
pub fn set_usage_enabled(state: State<'_, Arc<AppState>>, enabled: bool) {
    state.lock_config().usage.enabled = enabled;
}

/// The intent to open or collapse. Rust, not the webview, switches whether the
/// window takes clicks. tech.md 6.5 and 6.7.
#[tauri::command]
pub fn set_view(app: AppHandle, view: IslandView) {
    windows::set_view(&app, view);
}

/// Where typed text would go for this session right now. tech.md 6.5.
///
/// Read-only and cheap enough to call on every keystroke, which is the point:
/// panes close and sessions are abandoned, so a cached answer goes stale in
/// exactly the moment it matters.
#[tauri::command]
pub fn delivery_for(state: State<'_, Arc<AppState>>, session_id: String) -> Delivery {
    let Some(card) = state
        .sessions()
        .into_iter()
        .find(|c| c.session.session_id == session_id)
    else {
        return Delivery::Unreachable;
    };
    delivery_of(&card)
}

/// The ladder itself. tech.md 6.5.
fn delivery_of(card: &peekle_core::types::SessionCard) -> Delivery {
    if let Some(target) = card
        .session
        .pid
        .and_then(|pid| peekle_core::tmux::Tmux::find().and_then(|tmux| tmux.pane_for(pid)))
    {
        return Delivery::Tmux(target);
    }

    // No pane, but a session that still turns will hit Stop, and that Stop is
    // an injection point. Only a session that has ended for good is out of
    // reach: its Stop is never coming. tech.md 6.5.
    match card.status {
        peekle_core::types::SessionStatus::Ended => Delivery::Unreachable,
        _ => Delivery::TurnBoundary,
    }
}

/// Sends what the user typed, by the best channel this session has.
///
/// The reply lands in the feed first and stays `Running` until something
/// confirms it: `UserPromptSubmit` for a pane, the Stop that carries it for a
/// queue. tmux reports success even for a pane whose agent has exited, so its
/// return value is never the confirmation. tech.md 6.3.
#[tauri::command]
pub fn send_message(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    text: String,
) -> Delivery {
    let text = text.trim();
    if text.is_empty() {
        return Delivery::Unreachable;
    }

    let Some(card) = state
        .sessions()
        .into_iter()
        .find(|c| c.session.session_id == session_id)
    else {
        tracing::warn!(session_id, "a reply for a session nobody knows");
        return Delivery::Unreachable;
    };

    let delivery = delivery_of(&card);

    // Into the feed before anything is attempted: a message the user cannot
    // see is a message they will type twice. tech.md 6.5.
    let cards = state.user_turn(
        &card.session,
        text,
        peekle_core::types::EntryState::Running,
        now_ms(),
    );
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }

    match &delivery {
        Delivery::Tmux(target) => {
            let sent = peekle_core::tmux::Tmux::find().is_some_and(|tmux| tmux.send(target, text));
            if !sent {
                tracing::warn!(session_id, "tmux refused the keys");
                fail_replies(&app, state.inner(), &session_id);
            }
        }
        Delivery::TurnBoundary => match state.queue_reply(&session_id, text) {
            crate::state::Queued::LeftNow => {
                tracing::debug!(session_id, "a held turn carried the reply");
            }
            crate::state::Queued::Waiting => {
                tracing::debug!(session_id, "reply waits for the next stop");
            }
        },
        Delivery::Unreachable => {
            tracing::warn!(session_id, "nowhere to deliver");
            fail_replies(&app, state.inner(), &session_id);
        }
    }

    delivery
}

/// A message that will never leave says so rather than sitting dim forever.
fn fail_replies(app: &AppHandle, state: &Arc<AppState>, session_id: &str) {
    let cards = state.replies_failed(session_id, now_ms());
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
}

/// Renames a session in the island. tech.md 6.5.
///
/// An override, not an edit: hooks and the backfill rebuild a card on every
/// event, so a title written into one would be gone by the next tool call. An
/// empty title hands the name back to whatever the hooks derived.
#[tauri::command]
pub fn rename_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    title: String,
) {
    let Some(cards) = state.rename_session(&session_id, &title) else {
        tracing::warn!(session_id, "renamed a session nobody knows");
        return;
    };
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
}

/// Puts a session away for good.
///
/// Hides, never deletes: the transcript belongs to Claude Code, Peekle only
/// reads it, and the session stays where the user can still find it there.
/// tech.md 11.
#[tauri::command]
pub fn hide_session(app: AppHandle, state: State<'_, Arc<AppState>>, session_id: String) {
    let cards = state.hide_session(&session_id);
    tracing::debug!(session_id, "session hidden from the island");

    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
}

/// The webview reports the size of the shape it drew. Rust never resizes the
/// window with it: letting the frontend drive the frame is exactly the stutter
/// 6.7 forbids. What it does do is remember the size, because that rectangle is
/// where a resting island takes its click and what an open one has to be walked
/// away from. tech.md 6.7.
#[tauri::command]
pub fn island_bounds(state: State<'_, Arc<AppState>>, width: f64, height: f64) {
    tracing::debug!(width, height, "island reported its bounds");
    state.set_shape_bounds((width, height));
}

/// The webview reports it painted its route. tech.md 6.5, added in core v3.
#[tauri::command]
pub fn window_ready(state: State<'_, Arc<AppState>>, label: String) {
    tracing::debug!(label, "webview reported ready");
    state.ready_gate(&label).notify_waiters();
}

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn dev_emit_prompt(
    app: AppHandle,
    request: peekle_core::types::PromptRequest,
) -> Result<(), String> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    if state.claim_prompt(request.clone()) {
        windows::open_prompt(&app, &request).await;
    }
    Ok(())
}
