//! Tauri commands. Names and payloads are the literal table of tech.md 6.5.
//! The frontend calls nothing else.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::types::{
    IslandView, PeekleState, PromptAnswer, PromptOutcome, SignInState, ToastRequest, ToastTone,
    UsageSnapshot,
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events;
use crate::notify::Notifier;
use crate::state::{AppState, HeldKind, HeldSettings};
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
            // time this returns, whichever way it was answered: a deny is read
            // and worked around, not obeyed as a stop. Saying idle would leave
            // the island still while work is happening, and hide the `Stop`
            // that is the way to end it. A dismissal or a timeout hands the
            // question back to the terminal instead. tech.md 6.3 and 6.5.
            let status = status_after(&outcome);
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

/// What a session is doing once its blocking prompt is settled. Any answer,
/// allow or deny, releases the hook and the agent runs; a dismissal or a
/// timeout hands the question back to the terminal. tech.md 6.3 and 6.5.
pub fn status_after(outcome: &PromptOutcome) -> peekle_core::types::SessionStatus {
    match outcome {
        PromptOutcome::Answered(_) => peekle_core::types::SessionStatus::Working,
        _ => peekle_core::types::SessionStatus::Idle,
    }
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

/// How long `claude auth status --json` is given to answer.
///
/// It reads a file and prints; it does not go to the network. A CLI that takes
/// longer than this is one that is not going to answer, and blocking a press
/// on it would be worse than not knowing.
const STATUS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Whether Claude Code itself thinks it is signed in.
///
/// `None` means the question could not be answered -- no binary, a CLI too old
/// for the command, output that is not JSON -- and it must never be read as a
/// "no": telling someone they are signed out on the strength of a parse
/// failure is exactly the pointless login this is here to prevent.
/// tech.md 6.16.
fn cli_signed_in() -> Option<bool> {
    let binary = peekle_core::claude_path()?;
    let mut child = std::process::Command::new(binary)
        .args(peekle_core::auth::STATUS_ARGS)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // Bounded by hand rather than by `output()`: a CLI that never returns
    // would otherwise hold this thread for the life of the process.
    let deadline = std::time::Instant::now() + STATUS_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                tracing::warn!("claude auth status did not answer in time");
                return None;
            }
            Err(err) => {
                tracing::warn!(error = %err, "could not wait on claude auth status");
                return None;
            }
        }
    }

    let mut raw = String::new();
    {
        use std::io::Read;
        child.stdout.take()?.read_to_string(&mut raw).ok()?;
    }
    // The outcome, never the body: it carries the account's email and org.
    let answer = peekle_core::auth::logged_in(&raw);
    tracing::debug!(?answer, "claude auth status");
    answer
}

/// Publishes a sign-in state and remembers it.
fn publish_sign_in(app: &AppHandle, next: SignInState) -> SignInState {
    // The stage, never the address and never the code. tech.md 6.16, rule 11.
    tracing::debug!(stage = ?next.stage, has_url = next.url.is_some(), "sign-in");
    if let Err(err) = app.emit(events::SIGN_IN, &next) {
        tracing::warn!(error = %err, "failed to emit the sign-in state");
    }
    next
}

/// Starts the sign-in Claude Code performs for itself. tech.md 6.16.
///
/// Only ever from a press. Nothing here raises the Keychain dialog -- that is
/// still `request_usage_access` alone (rule 12) -- and nothing here writes a
/// credential: the token this ends with is Claude Code's, written by Claude
/// Code, and Peekle goes on only reading it.
#[tauri::command]
pub async fn start_sign_in(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<SignInState, String> {
    let state = state.inner().clone();

    // Asked before the process is spawned, because the answer decides whether
    // spawning one is the right thing at all. A CLI that is signed in while
    // the endpoint refuses is not a login problem: the credential is there and
    // valid, and driving the user through `auth login` would repeat the
    // mistake Reconnect made under a new name. tech.md 6.16.
    if tauri::async_runtime::spawn_blocking(cli_signed_in)
        .await
        .ok()
        .flatten()
        == Some(true)
    {
        // Almost always the snapshot is simply old: Claude Code refreshes the
        // Keychain entry as it runs, and the reason on screen was written
        // before that happened. Asking again is the whole fix, and it is what
        // the press should have done rather than explaining a dead end. Only
        // if it still fails is there anything to say. tech.md 6.16.
        let fresh = fetch_usage(&app, &state).await;
        if fresh.reason.is_none() {
            return Ok(publish_sign_in(&app, SignInState::idle()));
        }
        return Ok(publish_sign_in(
            &app,
            SignInState::refused("Check your connection or VPN, then wait a moment."),
        ));
    }

    let Some(binary) = peekle_core::claude_path() else {
        return Ok(publish_sign_in(
            &app,
            SignInState::failed("no claude command found on this Mac"),
        ));
    };

    let host = state.sign_in().clone();
    let reporter = app.clone();
    let started = host.start(&binary, move |next| {
        publish_sign_in(&reporter, next);
    });

    match started {
        Ok(next) => Ok(publish_sign_in(&app, next)),
        Err(err) => {
            tracing::warn!(error = %err, "could not start the sign-in");
            Ok(publish_sign_in(
                &app,
                SignInState::failed("could not start claude auth login"),
            ))
        }
    }
}

/// Hands the code from the authorize page to the waiting process.
///
/// The code is a credential: only its length is ever logged, and it goes
/// straight into the process's stdin and nowhere else. tech.md 6.16, rule 11.
#[tauri::command]
pub async fn submit_sign_in_code(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    code: String,
) -> Result<SignInState, String> {
    let state = state.inner().clone();
    let host = state.sign_in().clone();

    let submitting = match host.submit_code(&code) {
        Ok(next) => next,
        Err(err) => {
            tracing::warn!(error = %err, "the sign-in code went nowhere");
            return Ok(publish_sign_in(&app, SignInState::failed(err.to_string())));
        }
    };
    publish_sign_in(&app, submitting);

    // The CLI has the code; whether it worked is a question for the CLI, not
    // for an exit status read through a terminal. Asked once it has had time
    // to write, and settled exactly once either way. Rule 10.
    let answered = tauri::async_runtime::spawn_blocking(cli_signed_in)
        .await
        .ok()
        .flatten();

    let settled = match answered {
        Some(true) => host.settle(true, None),
        Some(false) => host.settle(false, Some("that code was not accepted".into())),
        // The login may well have worked; we simply cannot say. Refreshing
        // below is what will tell, so this does not claim a failure.
        None => host.settle(true, None),
    };
    let settled = publish_sign_in(&app, settled);

    // The point of signing in is that the bars fill. Making the user press
    // again afterwards would be one more lost click. tech.md 6.16.
    if settled.stage == peekle_core::types::SignInStage::Done {
        fetch_usage(&app, &state).await;
    }
    Ok(settled)
}

/// Opens the authorize page in the user's browser.
///
/// Claude Code opens it itself; this is the way back when it did not -- a
/// different default browser, a refused `open`. It takes no argument on
/// purpose: the address comes from the running sign-in and from nowhere else,
/// so a page inside the webview cannot use this to open something of its own.
/// The island cannot link to it directly either -- an anchor in an overlay
/// webview would navigate the overlay. tech.md 6.16.
#[tauri::command]
pub fn open_sign_in_page(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let Some(url) = state.sign_in().state().url else {
        return Err("there is no sign-in page to open".to_string());
    };
    // The length, never the address: it carries `code_challenge` and `state`.
    tracing::debug!(url_len = url.len(), "opening the authorize page");
    std::process::Command::new("/usr/bin/open")
        .arg(&url)
        .spawn()
        .map_err(|err| {
            tracing::warn!(error = %err, "could not open the authorize page");
            "could not open your browser".to_string()
        })?;
    Ok(())
}

/// Kills the sign-in and puts the panel away. tech.md 6.16.
#[tauri::command]
pub fn cancel_sign_in(app: AppHandle, state: State<'_, Arc<AppState>>) {
    state.sign_in().cancel();
    publish_sign_in(&app, SignInState::idle());
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
    // Aimed, not started. A TUI brought up here would be typed into a few
    // seconds later, while it is still coming up, and a line written into a
    // TUI that is not ready disappears without a trace (v46.2). So the card
    // opens now and the agent starts with the first message, which it takes
    // as an argument and cannot miss. The same move `continue_session` makes.
    // tech.md 6.5.
    if !std::path::Path::new(&cwd).is_dir() {
        return Err("That folder does not exist".to_string());
    }
    let session = peekle_core::types::SessionRef {
        session_id: peekle_core::pty::new_session_id(),
        project: peekle_core::sessions::project_of(&cwd),
        cwd,
        pid: None,
        tty: None,
    };
    state.claim_session(&session.session_id);
    let cards = state.open_owned_session(session.clone(), now_ms());
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
    tracing::info!(session = %session.session_id, "aimed a session of our own");
    Ok(session)
}

/// The one refusal that means "wait, then ask again" rather than "no". The
/// webview matches these words. tech.md 6.5.
pub const BUSY_ELSEWHERE: &str = "That chat is open somewhere else right now";

/// Delivers a reply into an observed chat. tech.md 6.5.
///
/// Where it goes is the registry's call, made now rather than read off the
/// card: a live process that publishes an inbox takes the words itself, and
/// the chat stays its own -- the turn runs in the process Desktop or the IDE
/// has open, the answer lands in the one transcript both clients read. No
/// process means nobody holds the chat, and it is resumed in a pty of our
/// own at once. A live process with no inbox is the one honest "busy": the
/// webview keeps the text and asks again until the process goes or its
/// inbox appears.
///
/// Refused for a session the island already owns: it has a field already,
/// and a second process for it would be the two-agents race of v34.
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

    let message = peekle_core::shots::compose(text.trim(), &shots);
    let sessions_root = peekle_core::registry::default_root();
    let live = sessions_root
        .as_deref()
        .and_then(|root| peekle_core::registry::find_live(root, &session_id));

    let session = match peekle_core::registry::route(live) {
        peekle_core::registry::Route::Busy => return Err(BUSY_ELSEWHERE.to_string()),
        peekle_core::registry::Route::Inbox(live) => {
            let Some(root) = sessions_root.as_deref() else {
                return Err(BUSY_ELSEWHERE.to_string());
            };
            // A process that is alive and refuses is still a process that
            // is alive: never a reason to start a second one. The webview
            // asks again. tech.md 6.5.
            if let Err(err) = peekle_core::inbox::send(root, &live, &message) {
                tracing::warn!(
                    session = %session_id,
                    pid = live.pid,
                    error = %err,
                    "the live chat's inbox did not take the message"
                );
                return Err(BUSY_ELSEWHERE.to_string());
            }
            tracing::info!(
                session = %session_id,
                pid = live.pid,
                entrypoint = live.entrypoint.as_deref().unwrap_or("?"),
                "delivered into a live chat"
            );
            card.session.clone()
        }
        // The first message is handed to the spawn rather than typed into
        // it: a TUI that is still starting swallows a written line without
        // a trace. tech.md 6.5.
        peekle_core::registry::Route::Resume => {
            let held = state.take_settings(&session_id);
            spawn_owned(
                &app,
                state.inner(),
                card.session.clone(),
                true,
                &message,
                held,
            )?;
            card.session.clone()
        }
    };

    // Into the feed at once, the way a reply is: it is already on its way, and
    // a message the user cannot see is a message they will type twice.
    // `UserPromptSubmit` confirms it like any other. tech.md 6.3.
    if !message.is_empty() {
        let (cards, entry_id) = state.user_turn(
            &session,
            &message,
            peekle_core::types::EntryState::Running,
            now_ms(),
        );
        if let Err(err) = app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
        if let Some(entry_id) = entry_id {
            watch_delivery(&app, state.inner(), session.session_id.clone(), entry_id);
        }
    }

    Ok(session)
}

/// Gives a sent reply `delivery_confirm_secs` to be named by
/// `UserPromptSubmit`, then calls it undelivered. tech.md 6.3.
///
/// By id, so a reply sent later is not judged by an earlier one's clock, and
/// only from `Running`, so a reply confirmed in time is left alone. A row that
/// went red and is named after all is set right by the hook: the verdict was
/// early, not wrong forever. A reply that sits dim with nothing on the way is
/// the lie this closes -- the one a person waits on for a minute before
/// wondering.
fn watch_delivery(app: &AppHandle, state: &Arc<AppState>, session_id: String, entry_id: String) {
    /// How long a reply waits before it is nudged rather than given up on.
    ///
    /// Short, because the thing it cures is a message sitting in the input box
    /// typed and unsent, and every second of that is a person watching a grey
    /// bubble. Long enough that an ordinary confirmation, which takes well
    /// under a second, is never raced. tech.md 6.5.
    const NUDGE_AFTER: Duration = Duration::from_secs(3);

    let wait = Duration::from_secs(state.lock_config().behavior.delivery_confirm_secs as u64);
    let app = app.clone();
    let state = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(NUDGE_AFTER).await;
        // Still `Running` means no `UserPromptSubmit` named it, which is what
        // an unsent box looks like from here. One newline submits it if it is
        // there, and does nothing at all if it is not: nothing is written
        // twice, so no turn can be started twice. tech.md 6.5.
        if state.reply_waiting(&session_id, &entry_id) && state.nudge_session(&session_id) {
            tracing::info!(session = %session_id, "nudging a reply nothing has confirmed");
        }

        tokio::time::sleep(wait.saturating_sub(NUDGE_AFTER)).await;
        let Some(cards) = state.fail_reply(&session_id, &entry_id, now_ms()) else {
            return;
        };
        tracing::info!(session = %session_id, "a reply was never confirmed");
        if let Err(err) = app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
    });
}

/// Starts the `claude` of a session the island owns, with its first message.
///
/// The message rides as an argument and the picks made before the first turn
/// as `--model` and `--effort`: a TUI that is still coming up swallows what
/// is typed into it (v46.2), and an argument cannot be swallowed. Shared by
/// the first message of an aimed chat (`start_session`, then `send_message`)
/// and the resume of an observed one (`continue_session`). tech.md 6.5.
fn spawn_owned(
    app: &AppHandle,
    state: &Arc<AppState>,
    session: peekle_core::types::SessionRef,
    // Whether `session_id` names a chat that already has a transcript. A
    // resumed chat keeps its own id, which is the whole point: one
    // transcript, and every other client watching that id sees what Peekle
    // adds. tech.md 6.5.
    resume: bool,
    prompt: &str,
    held: HeldSettings,
) -> Result<(), String> {
    let Some(binary) = peekle_core::claude_path() else {
        return Err("Claude Code is not installed where Peekle can find it".to_string());
    };

    let (cols, rows) = {
        let config = state.lock_config();
        (config.behavior.pty_cols, config.behavior.pty_rows)
    };
    let spec = peekle_core::pty::SpawnSpec {
        session_id: session.session_id.clone(),
        cwd: session.cwd.clone(),
        cols,
        rows,
        resume,
        prompt: Some(prompt.to_string()),
        model: held.model,
        effort: held.effort,
        mode: held.mode,
        thinking: held.thinking,
    };

    state.claim_session(&spec.session_id);
    // Peekle set it, so Peekle is the one that can say what it is: nothing
    // reports thinking back. tech.md 6.20.
    let cards = state.note_thinking(&spec.session_id, spec.thinking.unwrap_or(true));
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }

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
        // The card stays: what failed is one start, and the next message
        // tries again. Only the process is forgotten.
        state.pty().forget(&spec.session_id);
        tracing::warn!(error = %err, "could not start a session");
        return Err(match err {
            peekle_core::pty::PtyError::NoCwd => "That folder does not exist".to_string(),
            other => other.to_string(),
        });
    }

    // The card before the caller opens it. The island shows this session
    // immediately, and the first hook is a whole agent startup away, so
    // without the card there is nothing on screen to draw. tech.md 6.5.
    let cards = state.open_owned_session(session, now_ms());
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }

    tracing::info!(session = %spec.session_id, "started a session of our own");
    Ok(())
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
    let (cards, entry_id) = state.user_turn(
        &card.session,
        text,
        peekle_core::types::EntryState::Running,
        now_ms(),
    );
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
    if let Some(entry_id) = entry_id {
        watch_delivery(&app, state.inner(), session_id.clone(), entry_id);
    }

    // A model or an effort picked before this session ever answered has been
    // waiting for exactly this write. tech.md 6.15.
    let held = state.take_settings(&session_id);

    // The first message of an aimed chat starts the agent, and rides as an
    // argument with the picks as flags: a TUI that is still coming up swallows
    // what is typed into it. tech.md 6.5.
    if !state.pty_running(&session_id) {
        return match spawn_owned(&app, state.inner(), card.session.clone(), false, text, held) {
            Ok(()) => Ok(()),
            Err(err) => {
                fail_replies(&app, state.inner(), &session_id);
                Err(err)
            }
        };
    }

    // A running process takes the picks as lines of their own, each a
    // `SETTING_GAP` ahead of the next, and the message last. tech.md 6.15.
    let owner = state.inner().clone();
    let id = session_id.clone();
    let body = text.to_string();
    let wrote = tauri::async_runtime::spawn_blocking(move || {
        for line in held.lines() {
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
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    model: String,
) -> Result<(), String> {
    let line = peekle_core::agent::model_command(&model)
        .map_err(|_| "That is not a model name".to_string())?;
    command_session(
        Some(&app),
        state.inner(),
        &session_id,
        Some((HeldKind::Model, model.trim().to_string())),
        &line,
    )
}

/// Sets the permission mode of a session Peekle owns. tech.md 6.19.
///
/// Two ways in, because the CLI offers exactly two. Before the session has
/// run, `--permission-mode` carries it on the spawn. After that the only
/// switch is the one a person uses: `Shift+Tab`, stepped along a cycle that
/// was measured on a live TUI rather than guessed -- manual, accept edits,
/// plan, auto, and back. `bypassPermissions` is not on that cycle, so no
/// number of presses can land a session in it. Where the session stands comes
/// from its own hooks; without that reading there is nothing to count from,
/// and the press is refused rather than guessed. tech.md 6.19.
#[tauri::command]
pub fn set_mode(
    state: State<'_, Arc<AppState>>,
    session_id: String,
    mode: peekle_core::types::PermissionMode,
) -> Result<(), String> {
    let state = state.inner();
    if !state.owns_session(&session_id) {
        return Err("Peekle can only set the mode of sessions it started".to_string());
    }

    // Nothing has run yet: the flag is exact, so it is used.
    if !state.session_has_answered(&session_id) {
        state.hold_setting(&session_id, HeldKind::Mode, mode.flag().to_string());
        return Ok(());
    }

    let Some(current) = state.session_mode(&session_id) else {
        return Err("Peekle has not seen which mode this session is in yet".to_string());
    };
    let Some(steps) = current.steps_to(mode) else {
        return Err("That mode is not on the cycle Shift+Tab walks".to_string());
    };

    tracing::debug!(session_id, ?current, ?mode, steps, "cycling the mode");
    state
        .pty()
        .cycle_mode(&session_id, steps)
        .map_err(|err| err.to_string())
}

/// Whether a session starts with thinking on. tech.md 6.20.
///
/// Before it starts and nowhere else. `MAX_THINKING_TOKENS=0` is an
/// environment variable, so it is read once by the process at startup; the
/// CLI offers no slash command and no key for it, and writing
/// `alwaysThinkingEnabled` into the user's own settings would change every
/// session on the machine rather than this one. tech.md 6.20.
#[tauri::command]
pub fn set_thinking(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    on: bool,
) -> Result<(), String> {
    let state = state.inner();
    if !state.owns_session(&session_id) {
        return Err("Peekle can only set this on sessions it starts".to_string());
    }
    if state.session_has_answered(&session_id) {
        return Err("Thinking is set when a session starts".to_string());
    }

    state.hold_setting(
        &session_id,
        HeldKind::Thinking,
        if on { "on" } else { "off" }.to_string(),
    );
    let cards = state.note_thinking(&session_id, on);
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
    Ok(())
}

/// Runs this session at ultracode: xhigh effort plus dynamic workflows.
///
/// Its own command because `--effort` does not take it -- the CLI's own help
/// lists `low, medium, high, xhigh, max` and nothing else -- while
/// `/effort ultracode` does, and says why: it holds for this session only.
/// tech.md 6.15.
#[tauri::command]
pub fn set_ultracode(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    command_session(
        Some(&app),
        state.inner(),
        &session_id,
        None,
        "/effort ultracode",
    )
}

/// Changes how hard the session is asked to think. tech.md 6.15.
#[tauri::command]
pub fn set_effort(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    effort: peekle_core::types::Effort,
) -> Result<(), String> {
    command_session(
        Some(&app),
        state.inner(),
        &session_id,
        Some((HeldKind::Effort, effort.flag().to_string())),
        &peekle_core::agent::effort_command(effort),
    )
}

/// Frees up context by summarising the conversation. The ring is the button.
/// tech.md 6.15.
#[tauri::command]
pub fn compact_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    command_session(
        Some(&app),
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
    // Present when there is a window to tell about the result. The tests call
    // this without one: what they check is the refusal, not the redraw.
    app: Option<&AppHandle>,
    state: &Arc<AppState>,
    session_id: &str,
    // What to hold if the session has not answered yet: the kind, and the
    // value as the CLI takes it -- a flag on the spawn or a line later.
    held: Option<(HeldKind, String)>,
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
        let Some((kind, value)) = held else {
            return Err("There is nothing to compact yet".to_string());
        };
        tracing::debug!(session_id, "holding a setting for the first message");
        state.hold_setting(session_id, kind, value);
        return Ok(());
    }

    state.pty().send(session_id, line).map_err(|err| {
        tracing::warn!(error = %err, session_id, "the pty refused the setting");
        "That session is no longer listening".to_string()
    })?;

    // Claude Code writes the change into its transcript, and that record is
    // what the feed shows (tech.md 6.15). Nothing fires for a slash command,
    // though, so an idle chat would show it only at the next hook. Read the
    // file again shortly, twice: once for a quick answer, once for a slow one.
    if let Some(app) = app {
        reread_soon(app, state, session_id);
    }
    Ok(())
}

/// The card's own `SessionRef`, for the paths that hold only an id.
fn session_for(state: &Arc<AppState>, session_id: &str) -> Option<peekle_core::types::SessionRef> {
    state
        .sessions()
        .into_iter()
        .find(|c| c.session.session_id == session_id)
        .map(|c| c.session)
}

/// Re-reads one session's transcript a moment from now, and again after that.
///
/// For the things Claude Code records without telling anyone: a slash command
/// raises no hook at all, so nothing would otherwise pull the file. Two reads
/// rather than a poll, because there is exactly one write to wait for.
fn reread_soon(app: &AppHandle, state: &Arc<AppState>, session_id: &str) {
    let Some(root) = peekle_core::transcripts::default_root() else {
        return;
    };
    let Some(card) = state
        .sessions()
        .into_iter()
        .find(|c| c.session.session_id == session_id)
    else {
        return;
    };
    let path = peekle_core::transcripts::transcript_path(
        &root,
        &card.session.cwd,
        &card.session.session_id,
    );

    let app = app.clone();
    let state = Arc::clone(state);
    let session_id = session_id.to_string();
    tauri::async_runtime::spawn(async move {
        for wait in [Duration::from_millis(900), Duration::from_millis(2500)] {
            tokio::time::sleep(wait).await;
            crate::hooks::refresh_session(
                app.clone(),
                Arc::clone(&state),
                session_id.clone(),
                path.to_string_lossy().into_owned(),
            );
        }
    });
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

/// What `stop_session` says when there is no turn to stop. tech.md 6.5.
pub const NOTHING_RUNNING: &str = "Nothing is running there";

/// Stops the running turn of a session. tech.md 6.5.
///
/// Our own session takes the key that interrupts a turn in the TUI, `Esc`,
/// written into its pty. A live process that is not ours has no interrupt
/// channel -- its inbox carries no such frame and a signal would end the
/// process, not the turn -- so it gets a request to stop, read at its next
/// tool boundary the way any queued line is. No feed row is added on that
/// path: the request arrives as a user turn and `UserPromptSubmit` adds it.
#[tauri::command]
pub fn stop_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    if state.owns_session(&session_id) {
        state.pty().interrupt(&session_id).map_err(|err| {
            tracing::warn!(session = %session_id, error = %err, "could not interrupt");
            NOTHING_RUNNING.to_string()
        })?;

        // Nothing reports an interrupted turn: `Stop` does not fire for one,
        // so the card would keep spinning `Working` until the stale sweep
        // gave up ten minutes later, and the button that ended the turn would
        // still be offering to end it. Measured live on 2026-09-08: the turn
        // is over 0.2s after the key. We pressed it, so we know. tech.md 6.5.
        let cards = state.set_session_status(
            &session_for(state.inner(), &session_id).unwrap_or_else(|| {
                peekle_core::types::SessionRef {
                    session_id: session_id.clone(),
                    cwd: String::new(),
                    project: String::new(),
                    pid: None,
                    tty: None,
                }
            }),
            peekle_core::types::SessionStatus::Idle,
            now_ms(),
        );
        if let Err(err) = app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
        return Ok(());
    }

    let sessions_root = peekle_core::registry::default_root();
    let live = sessions_root
        .as_deref()
        .and_then(|root| peekle_core::registry::find_live(root, &session_id));
    let (Some(root), peekle_core::registry::Route::Inbox(live)) =
        (sessions_root.as_deref(), peekle_core::registry::route(live))
    else {
        return Err(NOTHING_RUNNING.to_string());
    };
    peekle_core::inbox::send(root, &live, peekle_core::inbox::STOP_REQUEST).map_err(|err| {
        tracing::warn!(session = %session_id, pid = live.pid, error = %err, "stop request refused");
        NOTHING_RUNNING.to_string()
    })
}

/// The cards as they stand, for a webview that has just come up.
///
/// The list reaches the island by event, and an event sent before the webview
/// subscribed reaches nobody: `listen` is async, so the subscription lands
/// after the call that made it, while the transcript backfill emits from a
/// background thread the moment the app starts. Whoever won that race decided
/// whether a person saw their own dialogues -- and a lost race left "No
/// sessions yet" over a disk full of transcripts until some other agent's hook
/// happened to arrive. So there is a second, pulling path, asked for once on
/// mount. tech.md 6.1 and section 8.
#[tauri::command]
pub fn get_sessions(state: State<'_, Arc<AppState>>) -> Vec<peekle_core::types::SessionCard> {
    state.sessions()
}

/// Whether a resting island shows the percent on every ten. tech.md 6.18.
#[tauri::command]
pub fn usage_badge(state: State<'_, Arc<AppState>>) -> bool {
    state.lock_config().usage.badge
}

/// Turns the badge on or off.
///
/// Nothing to ask the system for and nothing that can be refused, so unlike
/// `set_notify_enabled` this one has no error to report. Showing the badge
/// once on the way in belongs to the island: the number it would show is on
/// screen already, and Rust has no business animating it. tech.md 6.18.
#[tauri::command]
pub fn set_usage_badge(state: State<'_, Arc<AppState>>, on: bool) {
    {
        // Scoped: `save_config` takes the same lock.
        state.lock_config().usage.badge = on;
    }
    state.save_config();
}

/// Whether a finished turn puts a banner on the screen. tech.md 6.17.
#[tauri::command]
pub fn notify_enabled(state: State<'_, Arc<AppState>>) -> bool {
    state.lock_config().notify.enabled
}

/// Turns the banner on or off, and shows the first one on the way in.
///
/// There is no separate permission to ask for on desktop: macOS raises its own
/// dialog on the first banner an app posts, so the first banner has to be a
/// consequence of the press. It is also the only honest confirmation the
/// switch can give -- the person sees the thing they just asked for, in the
/// place it will appear from now on. A system that swallows banners says
/// nothing about it, here or anywhere, which is what the line under the
/// switch is for. tech.md 6.17 and rule 12.
#[tauri::command]
pub fn set_notify_enabled(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    on: bool,
) -> Result<(), String> {
    {
        // Scoped: `save_config` takes the same lock, and holding it across the
        // call would deadlock the island on its own settings.
        state.lock_config().notify.enabled = on;
    }
    state.save_config();

    if !on {
        return Ok(());
    }
    crate::notify::SystemNotifier(&app)
        .post(crate::notify::TITLE, crate::notify::SWITCHED_ON)
        .map_err(|err| {
            tracing::warn!(error = %err, "the system refused the first notice");
            "macOS would not show a notification".to_string()
        })
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
            let refused = command_session(
                None,
                &state,
                "someone-elses",
                Some((HeldKind::Model, "opus".to_string())),
                line,
            );
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

        let pick = |kind: HeldKind, value: &str, line: &str| {
            command_session(None, &state, "ours", Some((kind, value.to_string())), line)
        };
        assert_eq!(pick(HeldKind::Model, "opus", "/model opus"), Ok(()));
        assert_eq!(pick(HeldKind::Effort, "max", "/effort max"), Ok(()));
        // Last pick of each kind wins: a model was chosen, not a sequence.
        assert_eq!(pick(HeldKind::Model, "haiku", "/model haiku"), Ok(()));

        let held = state.take_settings("ours");
        assert_eq!(held.model.as_deref(), Some("haiku"));
        assert_eq!(held.effort.as_deref(), Some("max"));
        assert_eq!(held.lines(), vec!["/model haiku", "/effort max"]);
        // Handed over once: they go on the wire with that message and nowhere
        // else.
        let again = state.take_settings("ours");
        assert!(again.model.is_none() && again.effort.is_none());
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

    /// `New session` aims a chat and starts nothing: the card is ours before
    /// any process runs, and the first message is what starts one. A TUI
    /// brought up first would be typed into while still coming up, and that
    /// line disappears without a trace (v46.2). tech.md 6.5.
    #[test]
    fn an_aimed_session_is_owned_before_any_process_runs() {
        let state = fresh();
        let session = peekle_core::types::SessionRef {
            session_id: "aimed".to_string(),
            cwd: "/tmp".to_string(),
            project: "tmp".to_string(),
            pid: None,
            tty: None,
        };
        state.claim_session(&session.session_id);
        state.open_owned_session(session, 1);

        assert!(state.owns_session("aimed"), "the field is live");
        assert!(!state.pty_running("aimed"), "and nothing runs yet");
    }

    /// One pick, two shapes: a flag on the spawn that carries the first
    /// message, a line into a pty that is already running. Taken once, gone
    /// after, whichever shape it took. tech.md 6.15.
    #[test]
    fn a_held_pick_is_a_flag_on_the_spawn_and_a_line_into_a_running_pty() {
        let state = fresh();
        state.hold_setting("s", HeldKind::Model, "opus".to_string());
        state.hold_setting("s", HeldKind::Effort, "high".to_string());

        let held = state.take_settings("s");
        assert_eq!(held.model.as_deref(), Some("opus"));
        assert_eq!(held.effort.as_deref(), Some("high"));
        assert_eq!(held.lines(), vec!["/model opus", "/effort high"]);

        let again = state.take_settings("s");
        assert!(again.model.is_none() && again.effort.is_none());
    }

    /// The pulling path hands over what the registry holds and nothing else:
    /// a webview that asks on mount and gets a different answer than the one
    /// the events carry would be a second source of truth. tech.md 6.1.
    #[test]
    fn the_cards_a_fresh_webview_asks_for_are_the_ones_the_registry_holds() {
        let state = fresh();
        assert!(state.sessions().is_empty(), "nothing has happened yet");

        let session = peekle_core::types::SessionRef {
            session_id: "s-1".to_string(),
            cwd: "/Users/mars/peekle".to_string(),
            project: "peekle".to_string(),
            pid: None,
            tty: None,
        };
        state.open_owned_session(session, 1);

        let handed = state.sessions();
        assert_eq!(handed.len(), 1);
        assert_eq!(handed[0].session.session_id, "s-1");
    }

    /// The one refusal the island waits out rather than reports. The webview
    /// matches it by text (`BUSY_ELSEWHERE` in `logic/sessions.ts`), so the
    /// words are a contract. tech.md 6.5.
    #[test]
    fn the_busy_refusal_reads_as_the_webview_expects() {
        assert_eq!(BUSY_ELSEWHERE, "That chat is open somewhere else right now");
    }

    /// A deny releases the hook like an allow does: the agent reads the
    /// refusal and carries on, so the card says Working and `Stop` is
    /// offered. Only a question handed back to the terminal leaves it idle.
    /// tech.md 6.5.
    #[test]
    fn any_answer_leaves_the_session_working_and_a_dismissal_leaves_it_idle() {
        use peekle_core::types::{PromptAnswer, SessionStatus};

        let deny_without_a_word = PromptOutcome::Answered(PromptAnswer {
            prompt_id: "p".to_string(),
            choice: Some("deny".to_string()),
            text: None,
            answers: Vec::new(),
        });
        assert_eq!(status_after(&deny_without_a_word), SessionStatus::Working);
        assert_eq!(status_after(&PromptOutcome::Dismissed), SessionStatus::Idle);
        assert_eq!(status_after(&PromptOutcome::TimedOut), SessionStatus::Idle);
        assert_eq!(status_after(&PromptOutcome::Bypassed), SessionStatus::Idle);
    }

    /// An interrupt is the one end of a turn nobody reports: `Stop` does not
    /// fire for it. We pressed the key, so the card is put to rest here --
    /// otherwise the button that just ended the turn would go on offering to
    /// end it, which is what "cancel does nothing" looked like. tech.md 6.5.
    #[test]
    fn a_session_we_interrupted_is_put_to_rest_by_the_press_that_did_it() {
        let state = fresh();
        let session = peekle_core::types::SessionRef {
            session_id: "ours".to_string(),
            cwd: "/tmp".to_string(),
            project: "tmp".to_string(),
            pid: None,
            tty: None,
        };
        state.claim_session(&session.session_id);
        state.open_owned_session(session.clone(), 1);
        state.set_session_status(&session, peekle_core::types::SessionStatus::Working, 2);
        assert_eq!(
            state.sessions()[0].status,
            peekle_core::types::SessionStatus::Working
        );

        // What `stop_session` does once the key is on the wire.
        let cards = state.set_session_status(&session, peekle_core::types::SessionStatus::Idle, 3);
        assert_eq!(cards[0].status, peekle_core::types::SessionStatus::Idle);
        assert_eq!(
            session_for(&state, "ours").map(|s| s.cwd),
            Some("/tmp".to_string())
        );
    }

    /// A stop for a session nobody runs is refused in words the webview shows.
    #[test]
    fn a_stop_with_nothing_running_is_refused_not_signalled() {
        let state = fresh();
        assert!(!state.owns_session("nobody"));
        assert_eq!(NOTHING_RUNNING, "Nothing is running there");
        // The pty host knows no such session, which is the owned path's refusal.
        assert!(matches!(
            state.pty().interrupt("nobody"),
            Err(peekle_core::pty::PtyError::NotOwned)
        ));
    }

    #[test]
    fn a_compact_before_the_first_answer_is_refused() {
        let state = fresh();
        state.claim_session("ours");
        assert_eq!(
            command_session(
                None,
                &state,
                "ours",
                None,
                peekle_core::agent::COMPACT_COMMAND
            ),
            Err("There is nothing to compact yet".to_string())
        );
    }
}
