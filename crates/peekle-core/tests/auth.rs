#![allow(clippy::unwrap_used)]
//! S20 acceptance for the parts that own a process. tech.md 6.16.
//!
//! Every end of a sign-in -- signed in from the browser, declined there, a
//! code pasted, a wrong code, a cancel -- reaches exactly one report and
//! leaves no process behind. The CLI is played by small shell scripts that
//! print the lines the 2.1.263 binary prints, through the real pty host: the
//! same bytes, the same exit codes, and no network.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use peekle_core::auth::{verdict, LoginReport, SignInHost};
use peekle_core::types::{SignInStage, SignInState};

const WAIT: Duration = Duration::from_secs(8);

#[test]
fn a_fresh_host_is_idle_and_holds_nothing() {
    let host = SignInHost::new();
    assert_eq!(host.state().stage, SignInStage::Idle);
    assert_eq!(host.state().url, None);
    assert!(!host.state().needs_code);
}

/// A press on Cancel with nothing running is not an error: the process may
/// have exited a moment earlier, and refusing there would leave the panel
/// open over nothing.
#[test]
fn cancelling_nothing_is_not_an_error() {
    let host = SignInHost::new();
    host.cancel();
    host.cancel();
    assert_eq!(host.state().stage, SignInStage::Idle);
}

/// The code goes into a process's stdin. With no process there is nowhere to
/// put it, and it must be refused rather than swallowed.
#[test]
fn a_code_with_nothing_to_receive_it_is_refused() {
    let host = SignInHost::new();
    assert!(host.submit_code("abc123").is_err());
    assert_eq!(host.state().stage, SignInStage::Idle);
}

#[test]
fn an_empty_code_never_reaches_the_process() {
    let host = SignInHost::new();
    assert!(host.submit_code("   ").is_err());
    assert!(host.submit_code("").is_err());
}

/// A verdict for a run that is no longer on screen settles nothing: a panel
/// the person closed must not reopen on a late answer. Rule 10.
#[test]
fn a_verdict_for_another_run_settles_nothing() {
    let host = SignInHost::new();
    let stale = host.generation() + 7;
    assert_eq!(host.settle(stale, SignInStage::Done, None), None);
    assert_eq!(host.state().stage, SignInStage::Idle);

    let current = host.generation();
    let settled = host.settle(current, SignInStage::Denied, None).unwrap();
    assert_eq!(settled.stage, SignInStage::Denied);
    assert_eq!(settled.url, None, "a finished run offers no page");
}

/// A binary that is not there fails at the spawn rather than leaving a host
/// that thinks something is running.
#[test]
fn a_missing_binary_leaves_nothing_running() {
    let host = Arc::new(SignInHost::new());
    let started = host.start(Path::new("/nonexistent/claude"), |_| {}, |_| {});

    assert!(started.is_err());
    assert!(
        host.submit_code("abc").is_err(),
        "a failed start must not leave a process to write to"
    );
}

/// The first half of every login, as the binary prints it: the line, the
/// manual address inside an OSC 8 hyperlink, the paste prompt.
const OPENING: &str = r#"printf 'Opening browser to sign in\342\200\246\r\n'
printf '\033]8;;https://claude.com/cai/oauth/authorize?code=true&state=t1\007https://claude.com/cai/oauth/authorize?code=true&state=t1\033]8;;\007\r\n'
printf 'Paste code here if prompted > '
"#;

/// Writes a fake `claude` that prints `OPENING` and then runs `rest`.
fn fake_cli(name: &str, rest: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("peekle-auth-{name}-{}", ulid::Ulid::generate()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("claude");
    std::fs::write(&path, format!("#!/bin/sh\n{OPENING}{rest}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

struct Run {
    host: Arc<SignInHost>,
    states: mpsc::Receiver<SignInState>,
    reports: mpsc::Receiver<LoginReport>,
}

fn run(binary: &Path) -> Run {
    let host = Arc::new(SignInHost::new());
    let (state_tx, states) = mpsc::channel();
    let (report_tx, reports) = mpsc::channel();
    host.start(
        binary,
        move |next| {
            let _ = state_tx.send(next);
        },
        move |report| {
            let _ = report_tx.send(report);
        },
    )
    .unwrap();
    Run {
        host,
        states,
        reports,
    }
}

/// Waits until the CLI stands at the paste prompt.
fn until_it_asks_for_a_code(host: &SignInHost) {
    let deadline = Instant::now() + WAIT;
    while !host.state().needs_code {
        assert!(Instant::now() < deadline, "the prompt never came");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The whole reason v83 exists: approving in the browser finishes the CLI on
/// its own, with no code, and the island has to hear that. tech.md 6.16.
#[cfg(unix)]
#[test]
fn approving_in_the_browser_finishes_without_a_code() {
    let cli = fake_cli(
        "approve",
        "sleep 0.3\nprintf '\\r\\nLogin successful.\\r\\n'\nexit 0",
    );
    let run = run(&cli);

    let report = run
        .reports
        .recv_timeout(WAIT)
        .expect("a report once the CLI exits");
    assert!(report.succeeded);
    assert!(!report.denied);
    assert_eq!(report.generation, run.host.generation());
    assert_eq!(verdict(&report, None).0, SignInStage::Done);

    let seen: Vec<SignInState> = run.states.try_iter().collect();
    assert!(
        seen.iter()
            .any(|state| state.stage == SignInStage::Waiting && state.url.is_some()),
        "the page address was on screen while it waited: {seen:?}"
    );
    assert!(
        run.reports
            .recv_timeout(Duration::from_millis(300))
            .is_err(),
        "one run, one report"
    );
}

#[cfg(unix)]
#[test]
fn declining_in_the_browser_is_a_refusal_not_a_failure() {
    let cli = fake_cli(
        "deny",
        "sleep 0.2\nprintf '\\r\\nLogin failed: access_denied: The user denied the request\\r\\n' >&2\nexit 1",
    );
    let run = run(&cli);

    let report = run.reports.recv_timeout(WAIT).unwrap();
    assert!(report.denied);
    assert!(!report.succeeded);
    assert_eq!(verdict(&report, Some(false)), (SignInStage::Denied, None));
}

/// The page showed a code, and the code went in through the field.
#[cfg(unix)]
#[test]
fn a_pasted_code_finishes_the_same_way() {
    let cli = fake_cli(
        "code",
        "IFS= read -r code\nif [ \"$code\" = abc123 ]; then printf 'Login successful.\\r\\n'; exit 0; fi\nprintf 'Login failed: Invalid code\\r\\n'; exit 1",
    );
    let run = run(&cli);
    until_it_asks_for_a_code(&run.host);

    let submitted = run.host.submit_code("abc123").unwrap();
    assert_eq!(submitted.stage, SignInStage::Finishing);

    let report = run.reports.recv_timeout(WAIT).unwrap();
    assert!(report.succeeded);
    assert_eq!(verdict(&report, Some(true)).0, SignInStage::Done);
}

#[cfg(unix)]
#[test]
fn a_wrong_code_says_why_and_never_repeats_the_code() {
    let cli = fake_cli(
        "wrong",
        "IFS= read -r code\nif [ \"$code\" = abc123 ]; then printf 'Login successful.\\r\\n'; exit 0; fi\nprintf 'Login failed: Invalid code\\r\\n'; exit 1",
    );
    let run = run(&cli);
    until_it_asks_for_a_code(&run.host);
    run.host.submit_code("nope-secret").unwrap();

    let report = run.reports.recv_timeout(WAIT).unwrap();
    let (stage, error) = verdict(&report, Some(false));
    assert_eq!(stage, SignInStage::Failed);
    let error = error.unwrap();
    assert_eq!(error, "Invalid code");
    assert!(
        !error.contains("nope-secret"),
        "the code never reaches the screen"
    );
}

/// A cancelled run reports nothing, even though its process dies right after.
/// Rule 10.
#[cfg(unix)]
#[test]
fn a_cancelled_run_reports_nothing() {
    let cli = fake_cli("cancel", "sleep 30");
    let run = run(&cli);
    until_it_asks_for_a_code(&run.host);

    run.host.cancel();
    assert_eq!(run.host.state().stage, SignInStage::Idle);
    assert!(
        run.reports
            .recv_timeout(Duration::from_millis(800))
            .is_err(),
        "a cancelled run must not settle anything"
    );
    assert!(
        run.host.submit_code("abc").is_err(),
        "nothing is left to write to"
    );
}

/// Rule 11 covers the token; 6.16 extends it word for word to the code from
/// the authorize page and to the address itself, which carries
/// `code_challenge` and `state`. Checked in the source, because the failure
/// mode is a line that is written once and then never looked at again.
#[test]
fn neither_the_code_nor_the_address_is_ever_logged() {
    let sources = [
        include_str!("../src/auth.rs"),
        include_str!("../src/account.rs"),
        include_str!("../../../src-tauri/src/commands.rs"),
    ];

    for source in sources {
        for line in source.lines() {
            let line = line.trim();
            if !line.starts_with("tracing::") {
                continue;
            }
            let fields: String = line.split('"').step_by(2).collect();
            for field in fields.split(',') {
                if !field.contains("code") && !field.contains("url") {
                    continue;
                }
                let value = field.trim();
                assert!(
                    value.ends_with(".len()") || value.ends_with(".is_some()"),
                    "a tracing field carries the code or the address itself: {line}"
                );
            }
        }
    }
}
