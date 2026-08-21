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

/// Holds a reply until the agent stops.
///
/// Not a send: there is no send. The text leaves as the body of the next
/// blocking hook of this session, which is the only channel there is, and the
/// entry stays `Running` in the feed until that happens. tech.md 6.5.
#[tauri::command]
pub fn queue_reply(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    text: String,
) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }

    let Some(card) = state
        .sessions()
        .into_iter()
        .find(|c| c.session.session_id == session_id)
    else {
        tracing::warn!(session_id, "queued a reply for a session nobody knows");
        return;
    };

    state.queue_reply(&session_id, text);
    let cards = state.user_turn(
        &card.session,
        text,
        peekle_core::types::EntryState::Running,
        now_ms(),
    );
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }

    // A working session will stop, and its stop carries the queue for free. A
    // standing one never will, so waiting for it means waiting forever. The
    // registry alone cannot tell those apart: a session open in an IDE sends
    // no hooks, reads as idle, and is being written by its own client right
    // now. The transcript's mtime is the signal that cannot lie. tech.md 6.5.
    const LIVE_CLIENT_WINDOW: std::time::Duration = std::time::Duration::from_secs(90);

    let working = card.status == peekle_core::types::SessionStatus::Working;
    let live_client = peekle_core::transcripts::default_root().is_some_and(|root| {
        peekle_core::transcripts::client_is_live(
            &root,
            &card.session.cwd,
            &session_id,
            LIVE_CLIENT_WINDOW,
        )
    });
    if working || live_client || state.is_resuming(&session_id) {
        tracing::debug!(
            session_id,
            working,
            live_client,
            "reply queued for the next stop"
        );
        return;
    }
    start_turn(&app, state.inner().clone(), &card.session);
}

/// Starts the one turn Peekle is allowed to start: the text the user typed for
/// a session nobody is working in. tech.md 2 and 6.5.
fn start_turn(app: &AppHandle, state: Arc<AppState>, session: &peekle_core::types::SessionRef) {
    let session_id = session.session_id.clone();
    if !state.claim_resume(&session_id) {
        return;
    }
    let Some(text) = state.take_queued(&session_id) else {
        state.release_resume(&session_id);
        return;
    };

    let Some(cli) = peekle_core::claude_path() else {
        tracing::warn!("no claude binary to resume with");
        fail_replies(app, &state, &session_id);
        state.release_resume(&session_id);
        return;
    };

    let cwd = session.cwd.clone();
    let handle = app.clone();
    let id = session_id.clone();
    tauri::async_runtime::spawn(async move {
        let app = handle;
        let session_id = id;
        tracing::info!(session = %session_id, "starting a turn for a standing session");

        let run = tauri::async_runtime::spawn_blocking({
            let session_id = session_id.clone();
            move || {
                std::process::Command::new(cli)
                    .current_dir(&cwd)
                    .args(["--resume", &session_id, "-p", &text])
                    // The island reports the run through its hooks, so the
                    // output of the process itself is of no use to anybody.
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
            }
        })
        .await;

        match run {
            Ok(Ok(status)) if status.success() => {
                tracing::debug!(session = %session_id, "the turn finished");
            }
            other => {
                tracing::warn!(session = %session_id, ?other, "the turn did not run");
                fail_replies(
                    &app,
                    &app.state::<Arc<AppState>>().inner().clone(),
                    &session_id,
                );
            }
        }
        app.state::<Arc<AppState>>().release_resume(&session_id);
    });

    // The text is the prompt of a run that is starting, so it has left.
    let cards = state.replies_delivered(&session_id, now_ms());
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
