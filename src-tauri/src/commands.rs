//! Tauri commands. Names and payloads are the literal table of tech.md 6.5.
//! The frontend calls nothing else.

use std::sync::Arc;

use peekle_core::types::{
    IslandView, PeekleState, PromptAnswer, PromptOutcome, ToastRequest, ToastTone, UsageSnapshot,
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

/// Shared by every path that settles a blocking prompt: an answer or a
/// dismissal from the UI, and a timeout the router gave up on. tech.md 6.5
/// and rule 10.
pub(crate) fn settle(
    app: &AppHandle,
    state: &Arc<AppState>,
    prompt_id: &str,
    outcome: PromptOutcome,
) {
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

        // `resolve_all` only settles the channels a hook is waiting on; it
        // never touches `active_prompt` or the queue, and nothing else does
        // either. Left alone, the panel keeps showing a permission nothing is
        // waiting on any more, and clicking it later finds `pending.resolve`
        // returning false and does nothing at all. tech.md rule 10.
        if let Some(request) = state.clear_prompts() {
            windows::close_prompt(app, &request.id, &PromptOutcome::Bypassed);
        }
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

/// Starts a session the island owns. tech.md 6.5.
///
/// The id is assigned here rather than read back later, so the very first hook
/// the process raises is already recognised as ours and lands on the right
/// card. The claim goes in before the spawn for the same reason: hooks can
/// arrive before this function returns.
#[tauri::command]
pub fn start_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    cwd: String,
) -> Result<peekle_core::types::SessionRef, String> {
    let Some(binary) = peekle_core::claude_path() else {
        return Err("Claude Code is not installed where Peekle can find it".to_string());
    };

    let (cols, rows) = {
        let config = state.lock_config();
        (config.behavior.pty_cols, config.behavior.pty_rows)
    };
    let spec = peekle_core::pty::SpawnSpec {
        session_id: peekle_core::pty::new_session_id(),
        cwd: cwd.clone(),
        cols,
        rows,
    };

    state.claim_session(&spec.session_id);

    let handle = app.clone();
    let owner = state.inner().clone();
    let result = state.pty().spawn(&binary, &spec, move |session_id| {
        // The process was ours, so this is the one place `Ended` states a fact
        // instead of guessing at someone else's session. tech.md 6.3.
        tracing::info!(session = %session_id, "the session we own has exited");
        owner.disown_session(&session_id);
        let cards = owner.mark_session_ended(&session_id, now_ms());
        if let Err(err) = handle.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
    });

    if let Err(err) = result {
        state.disown_session(&spec.session_id);
        tracing::warn!(error = %err, "could not start a session");
        return Err(match err {
            peekle_core::pty::PtyError::NoCwd => "That folder does not exist".to_string(),
            other => other.to_string(),
        });
    }

    let session = peekle_core::types::SessionRef {
        session_id: spec.session_id.clone(),
        cwd: cwd.clone(),
        project: peekle_core::sessions::project_of(&cwd),
        pid: None,
        tty: None,
    };

    // The card before the caller opens it. The island shows this session
    // immediately, and the first hook is a whole agent startup away, so
    // without the card there is nothing on screen to draw. tech.md 6.5.
    let cards = state.open_owned_session(session.clone(), now_ms());
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }

    tracing::info!(session = %spec.session_id, "started a session of our own");
    Ok(session)
}

/// Sends what the user typed into the session's pty. tech.md 6.5.
///
/// The reply lands in the feed first and stays `Running` until
/// `UserPromptSubmit` confirms it. A write to a pty succeeds even when the
/// process on the other end is gone, so its return value never confirms
/// anything. tech.md 6.3.
///
/// `shots` are paths of screenshots the user attached. They travel as lines of
/// the message, because Claude Code opens a file once it is named and needs
/// nothing else. The feed row carries the same composed text: that is what the
/// agent received, and showing anything else would show something that did not
/// happen. tech.md 6.13.
#[tauri::command]
pub fn send_message(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    text: String,
    shots: Vec<String>,
) -> Result<(), String> {
    let message = peekle_core::shots::compose(text.trim(), &shots);
    let text = message.as_str();
    if text.is_empty() {
        return Ok(());
    }

    let Some(card) = state
        .sessions()
        .into_iter()
        .find(|c| c.session.session_id == session_id)
    else {
        tracing::warn!(session_id, "a reply for a session nobody knows");
        return Err("That session is gone".to_string());
    };

    // Refuse rather than swallow. An observed session has no field to type in,
    // and text that vanishes silently is exactly the lie the old ladder told.
    if !state.owns_session(&session_id) {
        tracing::warn!(session_id, "a reply for a session the island does not own");
        return Err("Peekle can only talk to sessions it started".to_string());
    }

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

    if let Err(err) = state.pty().send(&session_id, text) {
        tracing::warn!(error = %err, session_id, "the pty refused the write");
        fail_replies(&app, state.inner(), &session_id);
        return Err("That session is no longer listening".to_string());
    }
    Ok(())
}

/// The webview opened or closed a screenshot at full size. tech.md 6.13.
///
/// No view change and no window resize: the picture is a layer over content
/// the island is already showing. All this buys is the pointer timer's right
/// to put the island away, which a picture the user opened by hand outranks.
#[tauri::command]
pub fn set_preview(state: State<'_, Arc<AppState>>, open: bool) {
    state.set_preview(open);
}

/// Ends a session the island owns.
#[tauri::command]
pub fn end_session(app: AppHandle, state: State<'_, Arc<AppState>>, session_id: String) {
    if !state.owns_session(&session_id) {
        tracing::debug!(session_id, "asked to end a session we do not own");
        return;
    }
    state.pty().end(&session_id);
    state.disown_session(&session_id);
    let cards = state.mark_session_ended(&session_id, now_ms());
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
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
    if state.set_shape_bounds((width, height)) {
        // A shape that shrank leaves a hand that never moved outside itself,
        // and that is the island moving rather than the user walking away.
        // Clearing the clock is not enough: the next one runs out just as
        // surely. So the pointer is pinned where it stands until it moves.
        // tech.md 6.7.
        state.pointer_returned();
        state.mark_shape_moved();
    }
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
