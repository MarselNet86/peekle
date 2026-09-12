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
use crate::platform;
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
/// timeout hands the question back to the terminal. An answer given in Claude
/// Code itself is still an answer: the tool has run, and calling that idle
/// would show a still island over a working agent. tech.md 6.3, 6.5 and 6.14.
pub fn status_after(outcome: &PromptOutcome) -> peekle_core::types::SessionStatus {
    match outcome {
        PromptOutcome::Answered(_) | PromptOutcome::AnsweredElsewhere => {
            peekle_core::types::SessionStatus::Working
        }
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
            session: None,
            text: if enabled {
                "Peekle is ON".to_string()
            } else {
                "Peekle is OFF".to_string()
            },
            detail: None,
            took_ms: None,
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
    let raw = cli_says(peekle_core::auth::STATUS_ARGS)?;
    // The outcome, never the body: it carries the account's email and org.
    let answer = peekle_core::auth::logged_in(&raw);
    tracing::debug!(?answer, "claude auth status");
    answer
}

/// Whether the Claude Code on this machine still carries the login the panel
/// drives. `None` when the question could not be put at all. tech.md 6.16.
fn cli_supports_login() -> Option<bool> {
    let help = cli_says(peekle_core::auth::HELP_ARGS)?;
    let answer = peekle_core::auth::supports_login(&help);
    tracing::debug!(answer, "claude carries the auth command");
    Some(answer)
}

/// Runs the CLI with `args` and hands back what it printed, or nothing.
///
/// Bounded by hand rather than by `output()`: a CLI that never returns would
/// otherwise hold this thread for the life of the process -- and a build
/// without the `auth` command does exactly that, because it reads the words as
/// a prompt and opens an interactive session on them. tech.md 6.16 and 6.27.
fn cli_says(args: &[&str]) -> Option<String> {
    let binary = peekle_core::claude_path()?;
    let mut command = std::process::Command::new(binary);
    let mut child = platform::hidden(&mut command)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let deadline = std::time::Instant::now() + STATUS_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                tracing::warn!(?args, "the claude command did not answer in time");
                return None;
            }
            Err(err) => {
                tracing::warn!(error = %err, ?args, "could not wait on the claude command");
                return None;
            }
        }
    }

    let mut raw = String::new();
    {
        use std::io::Read;
        child.stdout.take()?.read_to_string(&mut raw).ok()?;
    }
    Some(raw)
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
/// Why the endpoint said no, in the words that fit that reason. tech.md 6.16.
///
/// One sentence for every cause was the bug this fixes: a rate limit was
/// reported as a network problem, so the panel told somebody with a working
/// connection to check their connection. The causes are already told apart in
/// 6.4; this is only the place that stopped using the distinction.
fn refusal(snapshot: &UsageSnapshot) -> String {
    use peekle_core::types::UsageUnavailable;

    match snapshot.reason {
        Some(UsageUnavailable::RateLimited) => match snapshot.retry_after_ms {
            // The server named the wait, so the wait is named here: "a moment"
            // for twenty four minutes is a sentence that gets pressed again in
            // thirty seconds.
            Some(ms) if ms > 0 => format!(
                "Too many requests to the usage endpoint. It asked to wait {}.",
                about_now(ms)
            ),
            _ => "Too many requests to the usage endpoint. Give it a few minutes.".to_string(),
        },
        Some(UsageUnavailable::Offline) => {
            "Nothing answered at api.anthropic.com. Check your connection or VPN.".to_string()
        }
        Some(UsageUnavailable::Network) => {
            "The usage endpoint did not answer in time. Check your connection or VPN.".to_string()
        }
        // Everything else is the endpoint refusing a credential that Claude
        // Code says is good, which is not a thing the person can fix from
        // here beyond waiting.
        _ => "Claude Code is signed in and the endpoint refused anyway.".to_string(),
    }
}

/// A wait a person can act on: minutes once it is minutes, seconds below that.
fn about_now(ms: i64) -> String {
    let secs = ms / 1000;
    if secs < 90 {
        return format!("{secs} seconds");
    }
    let mins = (secs + 30) / 60;
    format!("about {mins} minutes")
}

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

    // Nothing to drive. The island does not ship a CLI and must not: it is
    // Anthropic's to install. So the panel stops explaining and hands over the
    // one line that fixes it. tech.md 6.16.
    if peekle_core::claude_path().is_none() {
        return Ok(publish_sign_in(
            &app,
            SignInState::needs(
                "Claude Code is not installed on this machine.",
                peekle_core::auth::install_fix(),
            ),
        ));
    }

    // Asked before anything else is put to the CLI, because everything else
    // this command runs is an `auth` subcommand: a build without one reads the
    // words as a prompt and opens an interactive session on them, which is a
    // sign-in that sits on `Starting` with no address and no exit. tech.md 6.16.
    if tauri::async_runtime::spawn_blocking(cli_supports_login)
        .await
        .ok()
        .flatten()
        == Some(false)
    {
        return Ok(publish_sign_in(
            &app,
            SignInState::needs(
                "This Claude Code is too old to sign in from the island.",
                peekle_core::auth::update_fix(),
            ),
        ));
    }

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
        return Ok(publish_sign_in(&app, SignInState::refused(refusal(&fresh))));
    }

    let Some(binary) = peekle_core::claude_path() else {
        return Ok(publish_sign_in(
            &app,
            SignInState::needs(
                "Claude Code is not installed on this machine.",
                peekle_core::auth::install_fix(),
            ),
        ));
    };

    let host = state.sign_in().clone();
    let reporter = app.clone();
    let watcher = state.clone();
    let watched = host.clone();
    let started = host.start(&binary, move |next| {
        let next = publish_sign_in(&reporter, next);
        // The process left on its own with no code in flight: the browser
        // finished the login through the CLI's own callback and there was
        // never a code to paste. Nobody else will settle this run, so its exit
        // does, the same way a submitted code is settled -- by asking the CLI
        // rather than trusting an exit status. tech.md 6.16.
        if next.stage == peekle_core::types::SignInStage::Finishing && !watched.is_running() {
            let app = reporter.clone();
            let state = watcher.clone();
            let host = watched.clone();
            tauri::async_runtime::spawn(async move {
                settle_from_cli(&app, &state, &host).await;
            });
        }
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
    Ok(settle_from_cli(&app, &state, &host).await)
}

/// Asks the CLI whether the run signed it in, and settles the run on the
/// answer. The one way a run ends well: after a code went in, and after the
/// process left on its own because the browser finished the login for it.
/// tech.md 6.16.
async fn settle_from_cli(
    app: &AppHandle,
    state: &Arc<AppState>,
    host: &peekle_core::auth::SharedSignInHost,
) -> SignInState {
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
    let settled = publish_sign_in(app, settled);

    // The point of signing in is that the bars fill. Making the user press
    // again afterwards would be one more lost click. tech.md 6.16.
    if settled.stage == peekle_core::types::SignInStage::Done {
        fetch_usage(app, state).await;
    }
    settled
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
    platform::open_url(&url).map_err(|err| {
        tracing::warn!(error = %err, "could not open the authorize page");
        "could not open your browser".to_string()
    })?;
    Ok(())
}

/// Kills the sign-in and puts the panel away. tech.md 6.16.
///
/// Async, and it has to be: a plain command runs on the main thread, and this
/// is the one that tears a pseudoconsole down. The teardown no longer blocks
/// its caller (`auth::put_down`), but the thread that draws the window is not
/// the place to find out. tech.md 6.27.
#[tauri::command]
pub async fn cancel_sign_in(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.sign_in().cancel();
    publish_sign_in(&app, SignInState::idle());
    Ok(())
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

/// Copies a chat somebody else is holding into one of our own, and answers
/// with the id it became.
///
/// The last route, and the only one that changes which chat the words land
/// in. Two processes resuming one id interleave into a single transcript --
/// the CLI documents exactly that -- so a chat that takes no messages is
/// copied instead. The original is untouched and its owner never learns of
/// the copy; the island opens the copy and says so. tech.md 6.5.
fn fork_session(
    app: &AppHandle,
    state: &Arc<AppState>,
    card: &peekle_core::types::SessionCard,
    message: &str,
) -> Result<peekle_core::types::SessionRef, String> {
    let mut session = card.session.clone();
    let from = session.session_id.clone();
    session.session_id = peekle_core::pty::new_session_id();
    session.pid = None;
    session.tty = None;

    tracing::info!(from = %from, into = %session.session_id, "forking a chat that is held elsewhere");
    let held = state.take_settings(&from);
    spawn_owned(
        app,
        state,
        session.clone(),
        false,
        Some(from),
        message,
        held,
    )?;
    Ok(session)
}

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
        // Somebody holds the chat and takes nothing: copy it rather than
        // refuse. Resuming in place would put two processes with two
        // histories into one transcript -- the interleaving the CLI documents
        // -- and refusing leaves a field that takes words and delivers none.
        // The original stays exactly where its owner left it. tech.md 6.5.
        peekle_core::registry::Route::Busy => {
            return fork_session(&app, state.inner(), &card, &message)
        }
        peekle_core::registry::Route::Inbox(live) => {
            let Some(root) = sessions_root.as_deref() else {
                return fork_session(&app, state.inner(), &card, &message);
            };
            if let Err(err) = peekle_core::inbox::send(root, &live, &message) {
                // The socket is there and would not take it. That is not a
                // reason to make the person type it again: copy the chat and
                // carry on in the copy. tech.md 6.5.
                tracing::warn!(
                    session = %session_id,
                    pid = live.pid,
                    error = %err,
                    "the live chat's inbox did not take the message, forking instead"
                );
                return fork_session(&app, state.inner(), &card, &message);
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
                None,
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

/// The screenshot the user pasted, saved as a file. tech.md 6.13.
///
/// The pill's own path, entered from the other end. Pressing the attach key
/// and pressing `⌘V` are the same act -- a person saying "take this image" --
/// so the same thing happens to the image, and the agent cannot tell the two
/// apart: a path on a line before the text.
///
/// `None` when the pasteboard holds no image, which is not a failure: the
/// webview asked because the clipboard's types looked like one, and the types
/// are a description, not a promise.
fn save_paste(state: &Arc<AppState>, dir: &std::path::Path) -> Option<String> {
    // The only read of the pasteboard's contents besides the pill's, and like
    // that one it happens after the person acted. From macOS 15 this is what
    // raises the system paste prompt, and a prompt on `⌘V` is the prompt that
    // action asks for. tech.md 6.13 and R-13.
    let png = state.pasteboard.read_png()?;
    let keep = state.lock_config().shots.keep;
    let id = peekle_core::shots::new_id();

    match peekle_core::shots::write_shot(dir, &id, &png, keep) {
        Ok(path) => Some(path.to_string_lossy().to_string()),
        Err(err) => {
            tracing::warn!(error = %err, "could not save a pasted screenshot");
            None
        }
    }
}

/// `⌘V` in the field, when what is on the pasteboard is a picture. tech.md 6.13.
#[tauri::command]
pub fn paste_shot(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<Option<String>, String> {
    let Some(dir) = peekle_core::shots::shots_dir() else {
        return Err("Nowhere to save the screenshot".to_string());
    };
    let Some(path) = save_paste(state.inner(), &dir) else {
        return Ok(None);
    };

    tracing::info!(session = %session_id, "pasted a screenshot into the field");
    // The same event the attach key raises, so the attachment arrives in the
    // field by one route however it got here.
    let payload = serde_json::json!({ "session_id": session_id, "path": path });
    if let Err(err) = app.emit_to(crate::platform::ISLAND, events::SHOT_ATTACHED, payload) {
        tracing::warn!(error = %err, "failed to emit shot-attached");
    }
    Ok(Some(path))
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
        // Nothing is judged, and nothing is nudged, while the CLI has its
        // question about the folder on screen: it runs no prompt until that is
        // answered, so the reply is not late, it has not been offered yet. The
        // nudge would be worse than pointless -- a newline there lands on the
        // question's own `No, exit`, which is under the cursor. tech.md 6.24.
        while state.asking_trust(&session_id) {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
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
    // The chat this one copies, when it is a fork. tech.md 6.5.
    fork_from: Option<String>,
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
        fork_from,
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
    // The CLI asks about the folder before it runs anything, and asks it on
    // screen: no hook fires and no transcript is written until it is answered.
    // So the island puts the question where the person is, and nothing about
    // this chat is given up on while it stands. tech.md 6.24.
    let asking = app.clone();
    let asked = state.clone();
    let result = state.pty().spawn(
        &binary,
        &spec,
        move |session_id| {
            // The process was ours, so this is the one place `Ended` states a
            // fact instead of guessing at someone else's session. tech.md 6.3.
            tracing::info!(session = %session_id, "the session we own has exited");
            owner.disown_session(&session_id);
            let cards = owner.mark_session_ended(&session_id, now_ms());
            if let Err(err) = handle.emit(events::SESSIONS, &cards) {
                tracing::warn!(error = %err, "failed to emit sessions");
            }
        },
        move |session_id| {
            tracing::info!(session = %session_id, "the CLI is asking about this folder");
            let Some(cards) = asked.ask_trust(&session_id, now_ms()) else {
                return;
            };
            if let Err(err) = asking.emit(events::SESSIONS, &cards) {
                tracing::warn!(error = %err, "failed to emit sessions");
            }
        },
    );

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
        return match spawn_owned(
            &app,
            state.inner(),
            card.session.clone(),
            false,
            None,
            text,
            held,
        ) {
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
            // Each pick answers its own question before the next one goes, or
            // the line after it lands in a dialog. tech.md 6.15.
            write_command(&owner, &id, &line)?;
            std::thread::sleep(peekle_core::pty::SETTING_GAP);
        }
        // And a slash command typed into the field is a slash command: it
        // raises the same dialogs the row does, and until v80 it was the one
        // way left to fall into the bug the row was fixed for. tech.md 6.15.
        if peekle_core::pty::is_slash_command(&body) {
            write_command(&owner, &id, &body)
        } else {
            owner.pty().send(&id, &body)
        }
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
pub async fn set_model(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    model: String,
) -> Result<(), String> {
    let line = peekle_core::agent::model_command(&model)
        .map_err(|_| "That is not a model name".to_string())?;
    command_off_thread(
        app,
        state.inner().clone(),
        session_id,
        Some((HeldKind::Model, model.trim().to_string())),
        line,
    )
    .await
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
pub async fn set_ultracode(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    command_off_thread(
        app,
        state.inner().clone(),
        session_id,
        None,
        "/effort ultracode".to_string(),
    )
    .await
}

/// Changes how hard the session is asked to think. tech.md 6.15.
#[tauri::command]
pub async fn set_effort(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    effort: peekle_core::types::Effort,
) -> Result<(), String> {
    command_off_thread(
        app,
        state.inner().clone(),
        session_id,
        Some((HeldKind::Effort, effort.flag().to_string())),
        peekle_core::agent::effort_command(effort),
    )
    .await
}

/// Frees up context by summarising the conversation. The ring is the button.
/// tech.md 6.15.
#[tauri::command]
pub async fn compact_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    command_off_thread(
        app,
        state.inner().clone(),
        session_id,
        None,
        peekle_core::agent::COMPACT_COMMAND.to_string(),
    )
    .await
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

    // `command`, not `send`: a setting the CLI cannot apply silently asks
    // about it first, and until that dialog is answered it eats whatever is
    // written next -- the message the person types after picking. tech.md 6.15.
    write_command(state, session_id, line).map_err(|err| {
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

/// `command_session` off the main thread. tech.md 6.15.
///
/// A sync Tauri command runs on the main thread, and this one sleeps: a
/// slash command is three writes over half a second, and a held setting
/// ahead of a message is another `SETTING_GAP` on top. Half a second of a
/// blocked main thread is half a second in which the island's own pointer
/// monitor -- the thing that opens and closes it -- receives nothing.
async fn command_off_thread(
    app: AppHandle,
    state: Arc<AppState>,
    session_id: String,
    held: Option<(HeldKind, String)>,
    line: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        command_session(Some(&app), &state, &session_id, held, &line)
    })
    .await
    .map_err(|err| {
        tracing::warn!(error = %err, "the setting never ran");
        "That session is no longer listening".to_string()
    })?
}

/// Whether a slash command written into this session may carry the newline
/// that answers a dialog. tech.md 6.15.
///
/// Only while no turn is running. The newline takes whatever option is under
/// the cursor, and while an agent works the cursor can be on a question the
/// CLI raised itself -- a permission prompt, drawn in the 400ms between the
/// command and its answer. Answering that on somebody's behalf is the one
/// thing this product must never do (rule 10), and a compact that has to be
/// confirmed by hand is a far smaller price. With the session at rest there
/// is no such question to hit: nothing is running that could ask one.
fn may_answer(state: &Arc<AppState>, session_id: &str) -> bool {
    state
        .sessions()
        .into_iter()
        .find(|card| card.session.session_id == session_id)
        .is_some_and(|card| card.status != peekle_core::types::SessionStatus::Working)
}

/// One line into a session's pty, answering its own dialog when it is safe to.
fn write_command(
    state: &Arc<AppState>,
    session_id: &str,
    line: &str,
) -> Result<(), peekle_core::pty::PtyError> {
    if may_answer(state, session_id) {
        state.pty().command(session_id, line)
    } else {
        state.pty().send(session_id, line)
    }
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

/// The webview says whether the field is being written in: the cursor in it
/// with the panel key, or a draft or an attachment waiting. tech.md 6.7.
///
/// No view change and no window resize, like `set_preview`: what it buys is
/// that the pointer timer does not put the island away under a hand that is
/// on the keyboard, and that no pill or other chat's turn writes over it.
#[tauri::command]
pub fn set_composing(state: State<'_, Arc<AppState>>, active: bool) {
    tracing::debug!(active, "the field is being written in, or not");
    state.set_composing(active);
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

/// Answers the CLI's question about the folder, as the person answered it in
/// the island. tech.md 6.24.
///
/// Yes writes the answer into the pty that asked; no ends the chat, which is
/// what the question's own `No, exit` does. Peekle never answers it by itself:
/// the CLI is asking whether this person vouches for what is in the folder,
/// and an overlay has nothing to say about that.
#[tauri::command]
pub fn answer_trust(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    trust: bool,
) -> Result<(), String> {
    if !state.asking_trust(&session_id) {
        return Err("Nothing is asking about a folder there".to_string());
    }
    if trust {
        if !state.trust_session(&session_id) {
            return Err("That chat is gone".to_string());
        }
        tracing::info!(session_id, "the folder was trusted, by the person");
        let Some(cards) = state.end_trust(&session_id) else {
            return Ok(());
        };
        if let Err(err) = app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
        return Ok(());
    }

    tracing::info!(session_id, "the folder was not trusted, so the chat ends");
    state.end_trust(&session_id);
    state.pty().end(&session_id);
    state.disown_session(&session_id);
    let cards = state.mark_session_ended(&session_id, now_ms());
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
    Ok(())
}

/// What `stop_session` says when there is no turn to stop. tech.md 6.5.
pub const NOTHING_RUNNING: &str = "Nothing is running there";

/// What it says about a chat another app holds and offers no way into.
///
/// Not `NOTHING_RUNNING`: something plainly is running there -- the process is
/// alive and its own window says so -- and telling a person nothing is running
/// while they watch it work is the kind of answer that makes them stop
/// believing the rest. tech.md 6.5.
pub const HELD_ELSEWHERE: &str = "Another app is holding that chat and takes no messages";

/// And when the process does publish an inbox but would not take the request.
pub const REFUSED_THE_STOP: &str = "That chat did not take the stop";

/// What a press on Stop is answered with when the request cannot be sent.
///
/// Three different facts, and until v80.7 all three said the same wrong one.
/// `Resume` is the only one where nothing is running: no process holds the
/// chat at all, so the turn the button was offering to stop is already over.
/// tech.md 6.5.
fn stop_refusal(route: &peekle_core::registry::Route) -> &'static str {
    match route {
        peekle_core::registry::Route::Resume => NOTHING_RUNNING,
        peekle_core::registry::Route::Busy => HELD_ELSEWHERE,
        // Reached only when the send itself failed: the inbox was there and
        // did not take it.
        peekle_core::registry::Route::Inbox(_) => REFUSED_THE_STOP,
    }
}

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
    let route = peekle_core::registry::route(live);
    let refusal = stop_refusal(&route);
    let over = matches!(route, peekle_core::registry::Route::Resume);
    let (Some(root), peekle_core::registry::Route::Inbox(live)) = (sessions_root.as_deref(), route)
    else {
        // Nothing holds the chat, so the turn the button offered to stop is
        // over -- and the card still says `Working`, which is what put the
        // button there. It is corrected here rather than left for the stale
        // sweep ten minutes out: we just looked, and we know. tech.md 6.5.
        if over {
            if let Some(session) = session_for(state.inner(), &session_id) {
                let cards = state.set_session_status(
                    &session,
                    peekle_core::types::SessionStatus::Idle,
                    now_ms(),
                );
                if let Err(err) = app.emit(events::SESSIONS, &cards) {
                    tracing::warn!(error = %err, "failed to emit sessions");
                }
            }
        }
        return Err(refusal.to_string());
    };
    peekle_core::inbox::send(root, &live, peekle_core::inbox::STOP_REQUEST).map_err(|err| {
        tracing::warn!(session = %session_id, pid = live.pid, error = %err, "stop request refused");
        REFUSED_THE_STOP.to_string()
    })?;

    // The press is answered here and not by the peer's hooks: the request is
    // gone, and until the turn ends the feed says so and the button takes no
    // second press. A second press is a second message and a second turn in
    // somebody's chat. tech.md 6.5.
    if let Some(cards) = state.start_stop(&session_id, now_ms()) {
        if let Err(err) = app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
    }
    Ok(())
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

/// Deletes a chat: the process, the transcript and the row.
///
/// It used to hide the row and do nothing else, and every consequence of that
/// was invisible on purpose: the chat went on being offered by
/// `claude --resume`, and a chat of ours went on running with no row for it,
/// its hooks dropped by the registry, nothing left to stop it with. So the
/// three go together now, in this order -- the process first, so that nothing
/// is still writing to the file that goes next. tech.md 6.26.
#[tauri::command]
pub fn delete_session(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    let card = state.card(&session_id);

    // Ours to end. Somebody else's process is not ours to kill and we have no
    // handle on it either; the row goes, the process is theirs.
    if state.owns_session(&session_id) {
        state.pty().end(&session_id);
        state.disown_session(&session_id);
        tracing::info!(session_id, "ended the process of a chat being deleted");
    }

    if let Some(path) = transcript_of(card.as_ref()) {
        if path.exists() {
            platform::to_trash(&path)?;
            tracing::info!(session_id, "the transcript went to the Trash");
        } else {
            tracing::debug!(session_id, "no transcript on disk to delete");
        }
    }

    // The id stays remembered: a straggling hook from a process somebody else
    // is running would otherwise raise the row again a second later.
    let cards = state.hide_session(&session_id);
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
    Ok(())
}

/// Where Claude Code keeps this chat, or `None` when there is no telling.
///
/// Derived from the cwd the way `transcripts::scan` derives it, rather than
/// taken from the hook payload: a card can come from the backfill, which never
/// saw a payload, and the two answer the same path. tech.md 6.11.
fn transcript_of(card: Option<&peekle_core::types::SessionCard>) -> Option<std::path::PathBuf> {
    let card = card?;
    let root = peekle_core::transcripts::default_root()?;
    Some(peekle_core::transcripts::transcript_path(
        &root,
        &card.session.cwd,
        &card.session.session_id,
    ))
}

/// The webview reports the size of the shape it drew. Rust never resizes the
/// window with it: letting the frontend drive the frame is exactly the stutter
/// 6.7 forbids. What it does do is remember the size, because that rectangle is
/// where a resting island takes its click and what an open one has to be walked
/// away from. tech.md 6.7.
#[tauri::command]
pub fn island_bounds(
    state: State<'_, Arc<AppState>>,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) {
    tracing::debug!(left, top, width, height, "island reported its bounds");
    if state.set_shape_bounds(peekle_core::island::Rect::new(left, top, width, height)) {
        // A shape that shrank leaves a hand that never moved outside itself,
        // and that is the island moving rather than the user walking away.
        // Clearing the clock is not enough: the next one runs out just as
        // surely. So the pointer is pinned where it stands until it moves.
        // tech.md 6.7.
        state.pointer_returned();
        state.mark_shape_moved();
    }
}

/// Raises the macOS folder dialog and answers with what was chosen.
///
/// `None` is a cancel, and a cancel is not an error: the person changed their
/// mind, which is not an event. The app takes the front for the length of the
/// dialog and gives it straight back -- a system dialog only comes up on the
/// active app, and one nobody can see is worse than none. tech.md 6.23.
#[tauri::command]
pub async fn choose_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // The pointer goes into the dialog, and that is not leaving the island:
    // the leave clock is off for as long as the dialog stands. tech.md 6.7
    // and 6.23.
    let state = app.state::<Arc<AppState>>().inner().clone();
    state.set_dialog(true);
    // AppKit only from the main thread, and this command is not on it.
    let _ = app.run_on_main_thread(platform::take_front);
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose the folder this chat works in")
        .pick_folder(move |picked| {
            let _ = tx.send(picked);
        });

    let picked = rx.await;
    state.set_dialog(false);
    let _ = app.run_on_main_thread(platform::give_front_back);
    let picked = picked.map_err(|_| "the folder dialog went away".to_string())?;

    let Some(folder) = picked else {
        tracing::debug!("the folder dialog was cancelled");
        return Ok(None);
    };
    let Ok(path) = folder.into_path() else {
        return Err("That folder cannot be reached".to_string());
    };
    let path = path.to_string_lossy().to_string();
    tracing::info!(folder = %path, "a folder was chosen");
    Ok(Some(path))
}

/// Raises the macOS file dialog and answers with what was chosen.
///
/// Files rather than a folder, and more than one of them: people attach a set.
/// Nothing is copied anywhere -- the file is already on disk and its path is
/// all the agent needs, so the path goes straight into the message the way a
/// screenshot's does. An empty list is a cancel, which is not an event.
/// tech.md 6.25.
#[tauri::command]
pub async fn choose_files(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // The pointer goes into the dialog, and that is not leaving the island.
    // tech.md 6.7 and 6.25.
    let state = app.state::<Arc<AppState>>().inner().clone();
    state.set_dialog(true);
    // AppKit only from the main thread, and this command is not on it.
    let _ = app.run_on_main_thread(platform::take_front);
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Attach files to this message")
        .pick_files(move |picked| {
            let _ = tx.send(picked);
        });

    let picked = rx.await;
    state.set_dialog(false);
    let _ = app.run_on_main_thread(platform::give_front_back);
    let picked = picked.map_err(|_| "the file dialog went away".to_string())?;

    let Some(files) = picked else {
        tracing::debug!("the file dialog was cancelled");
        return Ok(Vec::new());
    };
    let paths: Vec<String> = files
        .into_iter()
        .filter_map(|file| file.into_path().ok())
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    tracing::info!(files = paths.len(), "files were chosen");
    Ok(paths)
}

/// Every refusal names what is so, because each is a different thing: the
/// folder is gone, the chat is somebody else's, or the chat has already begun
/// and no `cd` reaches a running agent. tech.md 6.23.
fn aim_folder(
    state: &Arc<AppState>,
    session_id: &str,
    cwd: &str,
) -> Result<Vec<peekle_core::types::SessionCard>, String> {
    if !std::path::Path::new(cwd).is_dir() {
        return Err("That folder does not exist".to_string());
    }
    if !state.owns_session(session_id) {
        return Err("Peekle can only aim sessions it started".to_string());
    }
    if state.pty_running(session_id) {
        return Err("This chat is already running in its folder".to_string());
    }
    state
        .aim_session(session_id, cwd)
        .ok_or_else(|| "This chat has already begun".to_string())
}

/// Points an aimed chat at another folder. tech.md 6.23.
#[tauri::command]
pub fn set_session_cwd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: String,
    cwd: String,
) -> Result<(), String> {
    let cards = aim_folder(state.inner(), &session_id, &cwd)?;

    tracing::info!(session_id, cwd, "aimed a chat at another folder");
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }
    Ok(())
}

/// Where a bug goes. tech.md 6.22.
///
/// The address is here rather than in the webview for the reason
/// `open_sign_in_page` gives: a command that takes a URL from the page is a
/// command the page can point anywhere. A link in the markup is out for a
/// second reason -- an anchor in an overlay webview navigates the overlay.
pub const BUG_REPORT_URL: &str = "https://t.me/marselnet";

/// Opens the developer's Telegram. tech.md 6.22.
#[tauri::command]
pub fn open_bug_report() -> Result<(), String> {
    tracing::debug!("opening the bug report chat");
    platform::open_url(BUG_REPORT_URL).map_err(|err| {
        tracing::warn!(error = %err, "could not open the bug report chat");
        "could not open Telegram".to_string()
    })?;
    Ok(())
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

    /// Deletion has to find the file Claude Code writes, and it finds it the
    /// way the backfill does -- from the folder the chat runs in -- because a
    /// card can arrive from the backfill, which never saw a hook payload.
    /// tech.md 6.26 and 6.11.
    #[test]
    fn a_chat_is_deleted_from_the_path_its_folder_gives() {
        let card = peekle_core::types::SessionCard {
            session: peekle_core::types::SessionRef {
                session_id: "01JABC".into(),
                cwd: "/Users/dev/peekle".into(),
                project: "peekle".into(),
                pid: None,
                tty: None,
            },
            title: String::new(),
            status: peekle_core::types::SessionStatus::Idle,
            origin: peekle_core::types::SessionOrigin::Owned,
            entries: Vec::new(),
            agent: None,
            mode: None,
            thinking: None,
            compacting: None,
            stopping: None,
            asking_trust: None,
            updated_at: 0,
        };

        let path = transcript_of(Some(&card)).expect("a home to look under");
        assert!(
            path.ends_with("-Users-dev-peekle/01JABC.jsonl"),
            "{}",
            path.display()
        );
        assert_eq!(transcript_of(None), None, "a chat nobody knows has no file");
    }

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

    /// Three ways a stop cannot be sent, and three different facts. Saying
    /// "nothing is running there" about a chat another app is plainly running
    /// is the kind of answer that makes a person stop believing the rest.
    /// tech.md 6.5.
    #[test]
    fn a_stop_that_cannot_be_sent_says_which_of_the_three_it_was() {
        use peekle_core::registry::{LiveSession, Route};

        let held = LiveSession {
            pid: 42,
            session_id: "s1".to_string(),
            cwd: "/tmp/peekle".to_string(),
            proc_start: None,
            version: "2.1.263".to_string(),
            entrypoint: Some("claude-vscode".to_string()),
            peer_protocol: Some(1),
            inbox: Some(std::path::PathBuf::from("/tmp/cc-socks/42.sock")),
        };

        assert_eq!(stop_refusal(&Route::Resume), NOTHING_RUNNING);
        assert_eq!(stop_refusal(&Route::Busy), HELD_ELSEWHERE);
        assert_eq!(stop_refusal(&Route::Inbox(held)), REFUSED_THE_STOP);

        // And none of the three is the same sentence as another, or the
        // distinction is only in the code.
        let said = [NOTHING_RUNNING, HELD_ELSEWHERE, REFUSED_THE_STOP];
        let mut sorted = said.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), said.len());
    }

    /// The newline that answers a dialog goes only to a session at rest.
    ///
    /// While a turn runs, the cursor can be on a question the CLI raised
    /// itself, and taking the option under it would be this product allowing
    /// a tool on somebody's behalf -- the one thing rule 10 exists to stop.
    /// tech.md 6.15.
    #[test]
    fn a_dialog_is_answered_only_where_no_other_question_can_stand() {
        let state = fresh();
        let session = peekle_core::types::SessionRef {
            session_id: "mine".to_string(),
            cwd: "/tmp/peekle".to_string(),
            project: "peekle".to_string(),
            pid: None,
            tty: None,
        };
        state.claim_session(&session.session_id);
        state.open_owned_session(session.clone(), 1);

        state.set_session_status(&session, peekle_core::types::SessionStatus::Idle, 2);
        assert!(may_answer(&state, &session.session_id));

        state.set_session_status(&session, peekle_core::types::SessionStatus::Working, 3);
        assert!(!may_answer(&state, &session.session_id));

        // And a session nobody has a card for is not one to write into at all.
        assert!(!may_answer(&state, "nobody"));
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
        // The tool ran, so the agent is going. Calling it idle would leave a
        // still island over a working session. tech.md 6.14.
        assert_eq!(
            status_after(&PromptOutcome::AnsweredElsewhere),
            SessionStatus::Working
        );
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

    /// A pasted image becomes a file the same way an offered one does, and an
    /// empty pasteboard is silence rather than an error: the webview asked
    /// because the clipboard's types looked like a picture. tech.md 6.13.
    #[test]
    fn a_pasted_image_becomes_a_file_and_an_empty_pasteboard_becomes_nothing() {
        let board = Arc::new(peekle_core::shots::FakePasteboard::new());
        let state = Arc::new(AppState::new(
            Config::default(),
            Arc::new(FakeUsage::default()),
            board.clone(),
        ));
        let dir =
            std::env::temp_dir().join(format!("peekle-paste-{}", peekle_core::shots::new_id()));

        assert_eq!(save_paste(&state, &dir), None, "nothing on the pasteboard");

        board.write_screenshot(b"\x89PNG\r\n\x1a\nfake");
        let path = save_paste(&state, &dir).expect("a file for the pasted image");
        assert!(path.ends_with(".png"));
        assert_eq!(
            std::fs::read(&path).expect("the file is on disk"),
            b"\x89PNG\r\n\x1a\nfake"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The bug this fixes, in one line: a rate limit told the user to check
    /// a connection that was working. Every cause says its own thing now, and
    /// a named wait is named rather than called "a moment". tech.md 6.16.
    #[test]
    fn a_refusal_says_which_refusal_it_is() {
        use peekle_core::types::{UsageSource, UsageUnavailable};

        let refused = |reason, retry_after_ms| {
            refusal(&UsageSnapshot {
                windows: Vec::new(),
                source: UsageSource::Unavailable,
                reason: Some(reason),
                fetched_at: 0,
                keychain_granted: true,
                retry_after_ms,
            })
        };

        // Captured live: `retry-after: 1456`, which is what the panel has to
        // say instead of sending somebody to their router.
        let limited = refused(UsageUnavailable::RateLimited, Some(1_456_000));
        assert!(limited.contains("Too many requests"), "{limited}");
        assert!(limited.contains("about 24 minutes"), "{limited}");
        assert!(!limited.contains("connection"), "{limited}");

        let no_header = refused(UsageUnavailable::RateLimited, None);
        assert!(no_header.contains("few minutes"), "{no_header}");

        for reason in [UsageUnavailable::Offline, UsageUnavailable::Network] {
            let network = refused(reason, None);
            assert!(network.contains("connection or VPN"), "{network}");
        }
    }

    #[test]
    fn a_wait_reads_as_minutes_once_it_is_minutes() {
        assert_eq!(about_now(30_000), "30 seconds");
        assert_eq!(about_now(89_000), "89 seconds");
        assert_eq!(about_now(1_456_000), "about 24 minutes");
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

    /// tech.md 6.23. Three ways a folder cannot be taken, and each says which
    /// one it was: a chat that has begun is a different thing from a chat the
    /// island never started, and "no" alone teaches nobody which.
    #[test]
    fn a_folder_that_cannot_be_taken_says_why() {
        let state = fresh();
        let here = std::env::temp_dir();
        let here = here.to_string_lossy().to_string();

        let gone = aim_folder(&state, "ours", "/no/such/folder/here");
        assert_eq!(gone, Err("That folder does not exist".to_string()));

        // A chat somebody else runs has no folder of ours to point.
        let theirs = aim_folder(&state, "theirs", &here);
        assert_eq!(
            theirs,
            Err("Peekle can only aim sessions it started".to_string())
        );

        // Ours, aimed and empty: it moves.
        state.claim_session("ours");
        state.open_owned_session(
            peekle_core::types::SessionRef {
                session_id: "ours".to_string(),
                cwd: "/tmp/project".to_string(),
                project: "project".to_string(),
                pid: None,
                tty: None,
            },
            1,
        );
        let cards = aim_folder(&state, "ours", &here).expect("an aimed chat moves");
        assert_eq!(cards[0].session.cwd, here);

        // Said something, so the agent is already living in a folder.
        state.user_turn(
            &cards[0].session,
            "go on",
            peekle_core::types::EntryState::Running,
            2,
        );
        assert_eq!(
            aim_folder(&state, "ours", &here),
            Err("This chat has already begun".to_string())
        );
    }

    /// tech.md 6.22. The button goes to the developer and to nobody else, and
    /// the page it is pressed on has no say in that: the address is a constant
    /// here, and the command takes no argument that could carry another one.
    #[test]
    fn the_bug_button_carries_its_own_address() {
        assert_eq!(BUG_REPORT_URL, "https://t.me/marselnet");
        let _: fn() -> Result<(), String> = open_bug_report;
    }
}
