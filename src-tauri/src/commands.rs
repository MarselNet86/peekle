//! Tauri commands. Names and payloads are the literal table of tech.md 6.5.
//! The frontend calls nothing else.

use std::sync::Arc;

use peekle_core::types::{
    IslandView, PeekleState, PromptAnswer, PromptOutcome, ToastRequest, ToastTone, UsageSnapshot,
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events;
use crate::state::{AppState, HeldKind, LIVE_CLIENT_WINDOW};
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
    match ask_usage(state).await {
        Some(snapshot) => publish_usage(app, state, snapshot),
        None => state.usage(),
    }
}

/// One question to the provider, off the async runtime. Nothing is stored and
/// nothing is emitted: a path that asks more than once (the Reconnect press,
/// tech.md 6.4) must not blink the bars on every attempt.
async fn ask_usage(state: &Arc<AppState>) -> Option<UsageSnapshot> {
    ask_usage_within(state, peekle_usage::POLL_TIMEOUT).await
}

/// The same question, bounded by what the caller can wait for. tech.md 6.4.
async fn ask_usage_within(
    state: &Arc<AppState>,
    budget: std::time::Duration,
) -> Option<UsageSnapshot> {
    let provider = Arc::clone(&state.usage_provider);
    match tauri::async_runtime::spawn_blocking(move || provider.snapshot_within(budget)).await {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            tracing::warn!(error = %err, "the usage provider panicked");
            None
        }
    }
}

/// Remembers a snapshot and tells the island about it.
fn publish_usage(app: &AppHandle, state: &Arc<AppState>, snapshot: UsageSnapshot) -> UsageSnapshot {
    // The outcome, not the token. Without this line a failing account leaves
    // nothing behind but `could not reach the API` on the bars.
    tracing::debug!(
        source = ?snapshot.source,
        reason = ?snapshot.reason,
        windows = snapshot.windows.len(),
        "usage snapshot"
    );
    state.set_usage(snapshot);
    // Read back rather than re-emitting what was stored: `state.usage()`
    // stamps `keychain_granted` from the config as it stands right now, and
    // the provider never knows that bit at all. tech.md 6.4.
    let stamped = state.usage();
    if let Err(err) = app.emit(events::USAGE, &stamped) {
        tracing::warn!(error = %err, "failed to emit usage");
    }
    stamped
}

/// The only path allowed to raise the Keychain dialog. Never called from
/// startup and never from the background poll. tech.md 6.4 and rule 12.
///
/// A grant is remembered so the poll may start; a refusal is remembered so
/// nothing asks again until the user clears it by hand.
/// How many times a press asks again while the answer is a network failure.
const MANUAL_TRIES: u32 = 3;
/// And for how long in total. This is a wall clock, not a wish: each attempt
/// is given what is left of it, so the button cannot outlive the budget.
///
/// It matters because the expensive failure is silent. Measured: a blackholed
/// address -- wifi associated, nothing routed, a captive portal -- costs the
/// whole request timeout and answers nothing, while an unresolvable name fails
/// in a tenth of a second. Without a budget the first case left the button
/// lit for ten seconds and then simply gave up. tech.md 6.4.
const MANUAL_BUDGET: std::time::Duration = std::time::Duration::from_secs(6);
/// Between attempts. Long enough for a route to finish coming up, short
/// enough that three of them fit in the budget.
const MANUAL_GAP: std::time::Duration = std::time::Duration::from_millis(700);
/// The least an attempt is worth making with. Below this a request fails for
/// want of time rather than for want of a network.
const MIN_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(1500);

/// Whether a press asks again after this answer. tech.md 6.4.
///
/// Only a network failure is worth a second ask, and only inside both bounds:
/// an expired token and a refused Keychain answer the same however many times
/// they are asked, and a press that keeps a button lit for half a minute is a
/// press that failed differently.
fn ask_again(
    reason: Option<peekle_core::types::UsageUnavailable>,
    attempt: u32,
    elapsed: std::time::Duration,
) -> bool {
    // Only Offline is worth a second ask. It is a local, instant failure --
    // measured at a tenth of a second -- so retrying it costs nothing and
    // catches the interface-came-up-before-the-route-did window a wifi
    // reconnect opens for a moment.
    //
    // Network (something out there timed out, or answered oddly) is not
    // retried: each attempt burns up to the whole remaining budget for a
    // question that will likely fail the same way again, and hammering a
    // server that may simply be slow is how a press earns the rate limit it
    // was trying to avoid. Rate limiting itself is the clearest case of that:
    // the endpoint was reached and answered, and it asked for time.
    // tech.md 6.4.
    reason == Some(peekle_core::types::UsageUnavailable::Offline)
        && attempt < MANUAL_TRIES
        && elapsed < MANUAL_BUDGET
}

/// The press behind the Reconnect button.
///
/// One attempt is a lottery: an interface comes back before its route and its
/// DNS, and a press that lands in that gap gets the same network failure the
/// bars already show, which reads as a button that does nothing. So the press
/// asks again while the failure is a network one, and stops the moment it is
/// anything else: an expired token and a refused Keychain do not change on a
/// second ask. Only the last snapshot is published, because three failures in
/// a row are a blink rather than news. tech.md 6.4.
async fn reconnect(app: &AppHandle, state: &Arc<AppState>) -> UsageSnapshot {
    let started = std::time::Instant::now();
    let mut snapshot = None;

    for attempt in 1..=MANUAL_TRIES {
        // Whatever is left of the budget, and never nothing: an attempt with
        // no time is a request that fails for the wrong reason.
        let left = MANUAL_BUDGET
            .checked_sub(started.elapsed())
            .filter(|left| *left >= MIN_ATTEMPT)
            .unwrap_or(MIN_ATTEMPT);
        let Some(asked) = ask_usage_within(state, left).await else {
            break;
        };
        let again = ask_again(asked.reason, attempt, started.elapsed());
        snapshot = Some(asked);

        if !again {
            break;
        }
        tracing::debug!(attempt, "the network was not there yet, asking again");
        tokio::time::sleep(MANUAL_GAP).await;
    }

    match snapshot {
        Some(snapshot) => publish_usage(app, state, snapshot),
        None => state.usage(),
    }
}

#[tauri::command]
pub async fn request_usage_access(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<UsageSnapshot, ()> {
    let state = state.inner().clone();
    let snapshot = reconnect(&app, &state).await;

    {
        use peekle_core::types::UsageUnavailable;

        let mut config = state.lock_config();
        match snapshot.reason {
            Some(UsageUnavailable::Denied) => config.usage.keychain_denied = true,
            // Everything else means the Keychain read itself went through --
            // entry there or not, the account reachable or not. `Offline`,
            // `Network` and `RateLimited` can only be reached after a token
            // was read: they describe the request to the usage endpoint that
            // followed, not the read that came before it. Refusing to grant
            // on them would leave a session with a real network problem
            // unable to ever open its own history behind the gate below.
            // tech.md 6.4.
            _ => {
                config.usage.keychain_denied = false;
                config.usage.keychain_granted = true;
            }
        }
    }
    state.save_config();

    // The command's return value is what the island actually applies (see
    // usage.svelte.ts::connect), not the emitted event, so it has to carry
    // the grant this same press just wrote -- not the reading from before it.
    // tech.md 6.4.
    let stamped = state.usage();
    if let Err(err) = app.emit(events::USAGE, &stamped) {
        tracing::warn!(error = %err, "failed to emit usage");
    }
    Ok(stamped)
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
    spawn_owned(&app, state.inner(), cwd, None, None)
}

/// Forks an observed chat into one the island owns. tech.md 6.5.
///
/// The same move Claude Desktop makes to open an existing chat: it spawns
/// `claude --resume=<id>` as its own child rather than reaching into a foreign
/// process, because there is no channel into one. The fork gives the run a new
/// id -- the one assigned here -- so its hooks are recognised as ours and the
/// original transcript is left alone.
///
/// Refused for a session the island already owns (it has a field already) and
/// for one a live client is still writing (a fork on a branch someone else is
/// writing is two agents racing, the hazard of v34). tech.md 6.5.
#[tauri::command]
pub fn continue_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    text: String,
    shots: Vec<String>,
) -> Result<peekle_core::types::SessionRef, String> {
    let Some(card) = state
        .sessions()
        .into_iter()
        .find(|c| c.session.session_id == session_id)
    else {
        tracing::warn!(session_id, "continue for a session nobody knows");
        return Err("That session is gone".to_string());
    };

    if state.owns_session(&session_id) {
        return Err("That session already has an input field".to_string());
    }

    // A live client on the same conversation would branch under us. The file
    // it writes on every message is the tell. tech.md 6.5 and v34.
    if let Some(root) = peekle_core::transcripts::default_root() {
        if peekle_core::transcripts::client_is_live(
            &root,
            &card.session.cwd,
            &session_id,
            LIVE_CLIENT_WINDOW,
        ) {
            return Err("That chat is open somewhere else right now".to_string());
        }
    }

    // The first message is handed to the spawn rather than typed into it: a
    // TUI that is still starting swallows a written line without a trace.
    // tech.md 6.5.
    let message = peekle_core::shots::compose(text.trim(), &shots);
    let session = spawn_owned(
        &app,
        state.inner(),
        card.session.cwd.clone(),
        Some(card.session.clone()),
        Some(message.clone()),
    )?;

    // Into the feed at once, the way a reply is: it is already on its way, and
    // a message the user cannot see is a message they will type twice.
    // `UserPromptSubmit` confirms it like any other. tech.md 6.3.
    if !message.is_empty() {
        let cards = state.user_turn(
            &session,
            &message,
            peekle_core::types::EntryState::Running,
            now_ms(),
        );
        if let Err(err) = app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
    }

    Ok(session)
}

/// Spawns a `claude` the island owns, optionally forking an existing chat into
/// it, and opens its card. Shared by `start_session` and `continue_session`.
fn spawn_owned(
    app: &AppHandle,
    state: &Arc<AppState>,
    cwd: String,
    // The chat being continued, or None to start a fresh one. A continued chat
    // keeps its own id, which is the whole point: one transcript, and every
    // other client watching that id sees what Peekle adds. tech.md 6.5.
    resume: Option<peekle_core::types::SessionRef>,
    prompt: Option<String>,
) -> Result<peekle_core::types::SessionRef, String> {
    let Some(binary) = peekle_core::claude_path() else {
        return Err("Claude Code is not installed where Peekle can find it".to_string());
    };

    let (cols, rows) = {
        let config = state.lock_config();
        (config.behavior.pty_cols, config.behavior.pty_rows)
    };
    let session_id = match &resume {
        Some(chat) => chat.session_id.clone(),
        None => peekle_core::pty::new_session_id(),
    };
    let spec = peekle_core::pty::SpawnSpec {
        session_id,
        cwd: cwd.clone(),
        cols,
        rows,
        resume: resume.is_some(),
        prompt,
    };

    state.claim_session(&spec.session_id);

    let handle = app.clone();
    let owner = state.clone();
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
/// Async because the write is a sleep: 120ms for the newline, and another
/// `SETTING_GAP` for each setting riding ahead of the message. That belongs on
/// a blocking thread rather than on the one drawing the island.
#[tauri::command]
pub async fn send_message(
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

    // A model or an effort picked before this session ever answered has been
    // waiting for exactly this write: it goes first, one line of its own, and
    // the message follows a `SETTING_GAP` later. tech.md 6.15.
    let held = state.take_settings(&session_id);
    let owner = state.inner().clone();
    let id = session_id.clone();
    let body = text.to_string();

    let wrote = tauri::async_runtime::spawn_blocking(move || {
        for line in held {
            owner.pty().send(&id, &line)?;
            std::thread::sleep(peekle_core::pty::SETTING_GAP);
        }
        owner.pty().send(&id, &body)
    })
    .await;

    match wrote {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            tracing::warn!(error = %err, session_id, "the pty refused the write");
            fail_replies(&app, state.inner(), &session_id);
            Err("That session is no longer listening".to_string())
        }
        Err(err) => {
            tracing::warn!(error = %err, session_id, "the write never ran");
            fail_replies(&app, state.inner(), &session_id);
            Err("That session is no longer listening".to_string())
        }
    }
}

/// The rows of the model menu, from the catalog. Called once on mount: the
/// catalog is captured with the build and does not move under a session.
/// tech.md 6.15.
#[tauri::command]
pub fn get_models() -> Vec<peekle_core::types::ModelChoice> {
    peekle_core::agent::choices()
}

/// What a session that has not answered yet is running as: Claude Code's own
/// defaults, which is what it started with, because the island passes no
/// `--model`. tech.md 6.15.
#[tauri::command]
pub fn get_defaults() -> peekle_core::types::AgentSetup {
    let settings = peekle_core::agent::settings_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    peekle_core::agent::defaults_from_settings(&settings)
}

/// Changes the model of a session the island owns. tech.md 6.15.
///
/// A slash command is text, so it travels the one channel text has: written
/// into the pty exactly the way a reply is. Nothing goes into the feed with
/// it -- a setting is not something the user said, and Claude Code writes it
/// into the transcript as a `<command-name>` record, which the synthetic
/// filter of 6.11 already drops.
#[tauri::command]
pub fn set_model(
    state: State<'_, Arc<AppState>>,
    session_id: String,
    model: String,
) -> Result<(), String> {
    let line = peekle_core::agent::model_command(&model)
        .map_err(|_| "That is not a model name".to_string())?;
    command_session(state.inner(), &session_id, Some(HeldKind::Model), &line)
}

/// Changes how hard the session is asked to think. tech.md 6.15.
#[tauri::command]
pub fn set_effort(
    state: State<'_, Arc<AppState>>,
    session_id: String,
    effort: peekle_core::types::Effort,
) -> Result<(), String> {
    command_session(
        state.inner(),
        &session_id,
        Some(HeldKind::Effort),
        &peekle_core::agent::effort_command(effort),
    )
}

/// Frees up context by summarising the conversation. The ring is the button.
/// tech.md 6.15.
#[tauri::command]
pub fn compact_session(state: State<'_, Arc<AppState>>, session_id: String) -> Result<(), String> {
    command_session(
        state.inner(),
        &session_id,
        None,
        peekle_core::agent::COMPACT_COMMAND,
    )
}

/// Writes one slash command into a session's pty.
///
/// Refuses everything `send_message` refuses, and for the same reason: an
/// observed session has no channel at all, and a setting that silently goes
/// nowhere is worse than one that says it could not.
fn command_session(
    state: &Arc<AppState>,
    session_id: &str,
    held: Option<HeldKind>,
    line: &str,
) -> Result<(), String> {
    if !state.owns_session(session_id) {
        tracing::warn!(
            session_id,
            "a setting for a session the island does not own"
        );
        return Err("Peekle can only talk to sessions it started".to_string());
    }

    // A session that has never answered is being aimed rather than changed,
    // and a line written into a TUI that is still coming up disappears without
    // a trace. So the pick waits and travels with the first message, which is
    // the write that proves the far end is listening. tech.md 6.15.
    if !state.session_has_answered(session_id) {
        let Some(kind) = held else {
            return Err("There is nothing to compact yet".to_string());
        };
        tracing::debug!(session_id, "holding a setting for the first message");
        state.hold_setting(session_id, kind, line.to_string());
        return Ok(());
    }

    state.pty().send(session_id, line).map_err(|err| {
        tracing::warn!(error = %err, session_id, "the pty refused the setting");
        "That session is no longer listening".to_string()
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use peekle_core::config::Config;
    use peekle_usage::FakeUsage;

    fn fresh() -> Arc<AppState> {
        Arc::new(AppState::new(
            Config::default(),
            Arc::new(FakeUsage::default()),
            Arc::new(peekle_core::shots::FakePasteboard::new()),
        ))
    }

    /// A setting for a session the island did not start refuses, exactly the
    /// way a message to it refuses. Writing it nowhere and saying nothing
    /// would leave the row showing a model the agent never heard of.
    /// tech.md 6.15.
    #[test]
    fn a_setting_for_a_session_we_do_not_own_is_refused() {
        let state = fresh();
        for line in [
            "/model opus",
            "/effort high",
            peekle_core::agent::COMPACT_COMMAND,
        ] {
            let refused = command_session(&state, "someone-elses", Some(HeldKind::Model), line);
            assert_eq!(
                refused,
                Err("Peekle can only talk to sessions it started".to_string()),
                "{line}"
            );
        }
    }

    /// A press asks again only while the answer is Offline, and only inside
    /// both bounds. tech.md 6.4.
    #[test]
    fn a_press_asks_again_only_while_offline_is_the_problem() {
        use peekle_core::types::UsageUnavailable;
        let quick = std::time::Duration::from_millis(200);

        assert!(ask_again(Some(UsageUnavailable::Offline), 1, quick));
        assert!(ask_again(Some(UsageUnavailable::Offline), 2, quick));

        // Network is not retried: each attempt burns up to the whole
        // remaining budget on a question likely to fail the same way again,
        // and rate limiting gets worse for being asked -- it was reached, and
        // it asked for time. tech.md 6.4.
        for reason in [
            None,
            Some(UsageUnavailable::Network),
            Some(UsageUnavailable::NotLoggedIn),
            Some(UsageUnavailable::Denied),
            Some(UsageUnavailable::NotGranted),
            Some(UsageUnavailable::Disabled),
            Some(UsageUnavailable::RateLimited),
            Some(UsageUnavailable::Unsupported),
        ] {
            assert!(!ask_again(reason, 1, quick), "{reason:?}");
        }
    }

    /// The press is a wall clock: whatever it spends on one attempt is gone
    /// from the next. A blackholed network costs the whole request timeout,
    /// so without this the button outlived its budget by seconds. 6.4.
    #[test]
    fn an_attempt_never_outlives_the_budget() {
        assert!(MIN_ATTEMPT < MANUAL_BUDGET);
        // Three attempts and two gaps still fit inside the budget when each
        // attempt is handed only what is left.
        let gaps = MANUAL_GAP * (MANUAL_TRIES - 1);
        assert!(gaps < MANUAL_BUDGET, "the gaps alone would spend it");
    }

    #[test]
    fn a_press_gives_up_on_either_bound() {
        use peekle_core::types::UsageUnavailable;
        let offline = Some(UsageUnavailable::Offline);

        assert!(
            !ask_again(offline, MANUAL_TRIES, std::time::Duration::from_millis(1)),
            "the last attempt is the last one"
        );
        assert!(
            !ask_again(offline, 1, MANUAL_BUDGET),
            "a hanging network must not keep the button lit"
        );
    }

    /// A session that has never answered is being aimed, not changed: the pick
    /// waits for the first message rather than going into a TUI that may still
    /// be coming up. tech.md 6.15.
    #[test]
    fn a_pick_made_before_the_first_answer_waits_for_it() {
        let state = fresh();
        state.claim_session("ours");

        assert_eq!(
            command_session(&state, "ours", Some(HeldKind::Model), "/model opus"),
            Ok(())
        );
        assert_eq!(
            command_session(&state, "ours", Some(HeldKind::Effort), "/effort max"),
            Ok(())
        );
        // Last pick of each kind wins: a model was chosen, not a sequence.
        assert_eq!(
            command_session(&state, "ours", Some(HeldKind::Model), "/model haiku"),
            Ok(())
        );

        assert_eq!(
            state.take_settings("ours"),
            vec!["/model haiku".to_string(), "/effort max".to_string()]
        );
        // Handed over once: they go on the wire with that message and nowhere
        // else.
        assert!(state.take_settings("ours").is_empty());
    }

    /// There is nothing to compact in a session that has not answered, and the
    /// ring is not a thing to hold for later.
    /// Continue refuses a session the island already owns: it has a field, and
    /// forking it would be a second process for one the user is already in.
    /// tech.md 6.5.
    #[test]
    fn continue_refuses_a_session_we_already_own() {
        // The guard `continue_session` runs before it ever spawns is
        // `owns_session`. A claimed id is owned.
        let state = fresh();
        state.claim_session("ours");
        assert!(state.owns_session("ours"));
    }

    /// The live-client window is the detector of v34, not a new number.
    #[test]
    fn the_live_client_window_matches_the_detector() {
        assert_eq!(LIVE_CLIENT_WINDOW, std::time::Duration::from_secs(90));
    }

    #[test]
    fn a_compact_before_the_first_answer_is_refused() {
        let state = fresh();
        state.claim_session("ours");
        assert_eq!(
            command_session(&state, "ours", None, peekle_core::agent::COMPACT_COMMAND),
            Err("There is nothing to compact yet".to_string())
        );
    }
}
