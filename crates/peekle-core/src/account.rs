//! What Claude Code on this Mac can do for the account. tech.md 6.16.
//!
//! Three questions to the CLI itself -- is there one, which version, does it
//! understand `auth status --json` -- and the command that fixes whatever the
//! answers say. The readings are free functions, so they are tested against
//! what the CLI prints without a `claude` binary; [`probe`] is the one place
//! that starts processes.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::types::{AccountLink, AccountState, CliState, InstallMethod};

/// The recommended install from Claude Code's own setup page. tech.md 6.16.
pub const INSTALL_COMMAND: &str = "curl -fsSL https://claude.ai/install.sh | bash";

/// How long either question is given. Both read local files and print; a CLI
/// that takes longer than this is not going to answer. tech.md 6.16.
pub const STATUS_TIMEOUT: Duration = Duration::from_secs(5);

const VERSION_ARGS: &[&str] = &["--version"];

/// Signs Claude Code out on this Mac. tech.md 6.16.
pub const LOGOUT_ARGS: &[&str] = &["auth", "logout"];

/// How long a sign-out is given. Longer than a status read: signing out may go
/// to the network to revoke the token. tech.md 6.16.
pub const LOGOUT_TIMEOUT: Duration = Duration::from_secs(15);

/// How the CLI begins the line it prints when a sign-out did not go through.
const LOGOUT_FAILED: &str = "Logout failed:";

/// The cask Homebrew installs when the path does not say which one.
const DEFAULT_CASK: &str = "claude-code";

/// The version in `claude --version`, which prints `2.1.263 (Claude Code)`.
///
/// The first word that is dotted digits, so a reworded suffix does not lose
/// the number.
pub fn version_of(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .find(|word| {
            let parts: Vec<&str> = word.split('.').collect();
            parts.len() >= 2
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(str::to_string)
}

/// What `claude auth status --json` came back with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusAnswer {
    /// JSON with a boolean `loggedIn`.
    Knows(bool),
    /// The process finished and printed something else: a CLI that does not
    /// know `auth`. tech.md 6.16.
    NotUnderstood,
    /// No answer inside the timeout, or no process at all. Says nothing about
    /// the CLI's age: a slow answer is not an old CLI.
    NoAnswer,
}

/// Reads the status output of a process that did or did not finish.
pub fn read_status(finished: bool, raw: &str) -> StatusAnswer {
    match crate::auth::logged_in(raw) {
        Some(signed_in) => StatusAnswer::Knows(signed_in),
        None if finished => StatusAnswer::NotUnderstood,
        None => StatusAnswer::NoAnswer,
    }
}

/// How `claude` was installed, read off where the binary really lives, and the
/// Homebrew cask when that is how. tech.md 6.16.
///
/// `path` should already be canonical: `/opt/homebrew/bin/claude` is a link,
/// and only its target says `Caskroom`.
pub fn install_method(path: &Path) -> (InstallMethod, Option<String>) {
    let text = path.to_string_lossy().replace('\\', "/");
    if let Some(at) = text.find("/Caskroom/") {
        let cask = text[at + "/Caskroom/".len()..]
            .split('/')
            .next()
            .filter(|cask| is_cask_name(cask))
            .map(str::to_string);
        return (InstallMethod::Homebrew, cask);
    }
    if text.contains("/node_modules/") {
        return (InstallMethod::Npm, None);
    }
    if text.contains("/.local/share/claude/") || text.ends_with("/.local/bin/claude") {
        return (InstallMethod::Native, None);
    }
    (InstallMethod::Unknown, None)
}

/// A cask name is copied into a command a person runs, so only the characters
/// a cask name is made of get through. tech.md 6.16.
fn is_cask_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"@._-".contains(&b))
}

/// The command that updates this install. tech.md 6.16.
pub fn update_command(method: InstallMethod, cask: Option<&str>) -> String {
    match method {
        InstallMethod::Homebrew => {
            format!("brew upgrade --cask {}", cask.unwrap_or(DEFAULT_CASK))
        }
        // Not `npm update -g`: it keeps to the range of the original install
        // and may not move to the newest release.
        InstallMethod::Npm => "npm install -g @anthropic-ai/claude-code@latest".to_string(),
        InstallMethod::Native | InstallMethod::Unknown => "claude update".to_string(),
    }
}

/// Where each account link goes. The addresses live here and nowhere else.
pub fn link_url(link: AccountLink) -> &'static str {
    match link {
        AccountLink::InstallGuide => "https://code.claude.com/docs/en/setup",
        AccountLink::UpdateGuide => "https://code.claude.com/docs/en/setup#update-claude-code",
        AccountLink::Changelog => {
            "https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md"
        }
    }
}

/// The answers put together. tech.md 6.16.
pub fn assemble(
    binary: Option<&Path>,
    version: Option<String>,
    status: StatusAnswer,
) -> AccountState {
    let Some(binary) = binary else {
        return AccountState {
            cli: CliState::Missing,
            version: None,
            install: InstallMethod::Unknown,
            signed_in: None,
            command: Some(INSTALL_COMMAND.to_string()),
        };
    };
    let (install, cask) = install_method(binary);
    match status {
        StatusAnswer::NotUnderstood => AccountState {
            cli: CliState::Outdated,
            version,
            install,
            signed_in: None,
            command: Some(update_command(install, cask.as_deref())),
        },
        StatusAnswer::Knows(signed_in) => AccountState {
            cli: CliState::Ready,
            version,
            install,
            signed_in: Some(signed_in),
            command: None,
        },
        StatusAnswer::NoAnswer => AccountState {
            cli: CliState::Ready,
            version,
            install,
            signed_in: None,
            command: None,
        },
    }
}

/// What a sign-out came to, from how the process ended and what it printed.
///
/// From the 2.1.263 binary: success prints `Successfully logged out…` and
/// exits zero, failure writes `Logout failed: <reason>` and exits one. The
/// exit code decides; the line only gives the reason words. tech.md 6.16.
pub fn logout_outcome(finished: bool, success: bool, output: &str) -> Result<(), String> {
    if !finished {
        return Err("Claude Code did not finish signing out.".to_string());
    }
    if success {
        return Ok(());
    }
    let reason = output
        .rfind(LOGOUT_FAILED)
        .map(|at| &output[at + LOGOUT_FAILED.len()..])
        .and_then(|rest| rest.lines().next())
        .map(|line| {
            line.split_whitespace()
                .filter(|word| !word.contains("://"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|reason| !reason.is_empty());
    Err(match reason {
        Some(reason) => format!("Claude Code could not sign out: {reason}"),
        None => "Claude Code could not sign out.".to_string(),
    })
}

/// Signs Claude Code out. Blocking, up to `LOGOUT_TIMEOUT`. tech.md 6.16.
pub fn logout() -> Result<(), String> {
    let binary = crate::claude_path().ok_or("Claude Code is not installed on this Mac.")?;
    let run = run_bounded(&binary, LOGOUT_ARGS, LOGOUT_TIMEOUT)
        .ok_or("Claude Code could not be started.")?;
    let outcome = logout_outcome(run.finished, run.success, &run.output);
    tracing::debug!(ok = outcome.is_ok(), "claude auth logout");
    outcome
}

/// Asks the CLI on this Mac. Blocking, up to two `STATUS_TIMEOUT`s; call it
/// off the async runtime. tech.md 6.16.
pub fn probe() -> AccountState {
    let Some(found) = crate::claude_path() else {
        return assemble(None, None, StatusAnswer::NoAnswer);
    };
    let binary = std::fs::canonicalize(&found).unwrap_or(found);

    let version =
        run_bounded(&binary, VERSION_ARGS, STATUS_TIMEOUT).and_then(|run| version_of(&run.output));
    let status = match run_bounded(&binary, crate::auth::STATUS_ARGS, STATUS_TIMEOUT) {
        Some(run) => read_status(run.finished, &run.output),
        None => StatusAnswer::NoAnswer,
    };
    let state = assemble(Some(&binary), version, status);
    // The outcome, never the output: status carries the email and the org.
    tracing::debug!(cli = ?state.cli, install = ?state.install, signed_in = ?state.signed_in, "account probe");
    state
}

/// How a bounded run of the CLI ended.
struct Run {
    /// It exited inside the time it was given.
    finished: bool,
    /// It exited with zero. Never true for a run that did not finish.
    success: bool,
    /// stdout, then stderr.
    output: String,
}

/// Runs `binary args` for at most `timeout`. `None` when it would not start.
fn run_bounded(binary: &Path, args: &[&str], timeout: Duration) -> Option<Run> {
    let mut child = Command::new(binary)
        .args(args)
        .env("PATH", crate::pty::session_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    // Read on threads, so a CLI that fills a pipe cannot hold itself up.
    let mut stdout = child.stdout.take()?;
    let mut stderr = child.stderr.take()?;
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let deadline = Instant::now() + timeout;
    let (finished, success) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (true, status.success()),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(40));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break (false, false);
            }
        }
    };
    let mut output = out.join().unwrap_or_default();
    output.push_str(&err.join().unwrap_or_default());
    Some(Run {
        finished,
        success,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Printed by the CLI on this Mac, 2026-09-14.
    #[test]
    fn reads_the_version_the_cli_prints() {
        assert_eq!(
            version_of("2.1.263 (Claude Code)\n").as_deref(),
            Some("2.1.263")
        );
        assert_eq!(version_of("claude 2.1.9").as_deref(), Some("2.1.9"));
        assert_eq!(version_of("(Claude Code)"), None);
        assert_eq!(version_of("1."), None);
        assert_eq!(version_of(""), None);
    }

    #[test]
    fn a_cli_that_answers_the_status_is_ready() {
        assert_eq!(
            read_status(true, r#"{"loggedIn":true}"#),
            StatusAnswer::Knows(true)
        );
        assert_eq!(
            read_status(true, r#"{"loggedIn":false}"#),
            StatusAnswer::Knows(false)
        );
    }

    /// Commander's refusal of a command it does not have. A CLI that finished
    /// and printed that is an old CLI. tech.md 6.16.
    #[test]
    fn a_cli_that_does_not_know_the_command_is_outdated() {
        assert_eq!(
            read_status(true, "error: unknown command 'auth'\n"),
            StatusAnswer::NotUnderstood
        );
    }

    /// A slow answer is not an old CLI.
    #[test]
    fn no_answer_says_nothing_about_age() {
        assert_eq!(read_status(false, ""), StatusAnswer::NoAnswer);
        assert_eq!(read_status(false, "partial"), StatusAnswer::NoAnswer);
    }

    /// The canonical path of the CLI on this Mac, 2026-09-14.
    #[test]
    fn reads_a_homebrew_install_and_its_cask_off_the_path() {
        let (method, cask) = install_method(Path::new(
            "/opt/homebrew/Caskroom/claude-code@latest/2.1.263/claude",
        ));
        assert_eq!(method, InstallMethod::Homebrew);
        assert_eq!(cask.as_deref(), Some("claude-code@latest"));
        assert_eq!(
            update_command(method, cask.as_deref()),
            "brew upgrade --cask claude-code@latest"
        );
    }

    #[test]
    fn reads_the_other_installs_off_the_path() {
        assert_eq!(
            install_method(Path::new("/Users/a/.local/share/claude/versions/2.1.263")).0,
            InstallMethod::Native
        );
        assert_eq!(
            install_method(Path::new("/Users/a/.local/bin/claude")).0,
            InstallMethod::Native
        );
        assert_eq!(
            install_method(Path::new(
                "/Users/a/.nvm/versions/node/v22/lib/node_modules/@anthropic-ai/claude-code/bin/claude"
            ))
            .0,
            InstallMethod::Npm
        );
        assert_eq!(
            install_method(Path::new("/usr/local/bin/claude")).0,
            InstallMethod::Unknown
        );
    }

    /// A cask name goes into a command a person pastes into a terminal, so a
    /// path that does not look like one falls back to the plain cask.
    #[test]
    fn a_strange_cask_never_reaches_the_command() {
        let (method, cask) =
            install_method(Path::new("/opt/homebrew/Caskroom/x;rm -rf ~/2.1/claude"));
        assert_eq!(method, InstallMethod::Homebrew);
        assert_eq!(cask, None);
        assert_eq!(
            update_command(method, None),
            "brew upgrade --cask claude-code"
        );
    }

    #[test]
    fn every_install_has_an_update_command() {
        assert_eq!(update_command(InstallMethod::Native, None), "claude update");
        assert_eq!(
            update_command(InstallMethod::Unknown, None),
            "claude update"
        );
        assert_eq!(
            update_command(InstallMethod::Npm, None),
            "npm install -g @anthropic-ai/claude-code@latest"
        );
    }

    #[test]
    fn no_cli_is_missing_and_carries_the_install_command() {
        let state = assemble(None, None, StatusAnswer::NoAnswer);
        assert_eq!(state.cli, CliState::Missing);
        assert_eq!(state.command.as_deref(), Some(INSTALL_COMMAND));
        assert_eq!(state.signed_in, None);
    }

    #[test]
    fn an_old_cli_carries_the_update_for_its_install() {
        let state = assemble(
            Some(Path::new("/opt/homebrew/Caskroom/claude-code/2.0.1/claude")),
            Some("2.0.1".into()),
            StatusAnswer::NotUnderstood,
        );
        assert_eq!(state.cli, CliState::Outdated);
        assert_eq!(state.version.as_deref(), Some("2.0.1"));
        assert_eq!(
            state.command.as_deref(),
            Some("brew upgrade --cask claude-code")
        );
    }

    /// "Do not know" never becomes "signed out". tech.md 6.16.
    #[test]
    fn a_cli_that_did_not_answer_is_ready_and_unknown() {
        let state = assemble(
            Some(Path::new("/usr/local/bin/claude")),
            None,
            StatusAnswer::NoAnswer,
        );
        assert_eq!(state.cli, CliState::Ready);
        assert_eq!(state.signed_in, None);
        assert_eq!(state.command, None);
    }

    /// The two endings the 2.1.263 binary has. Read out of the binary, not
    /// captured live: a live sign-out would sign the owner out. tech.md 6.16.
    #[test]
    fn a_sign_out_that_exited_zero_went_through() {
        assert_eq!(
            logout_outcome(
                true,
                true,
                "Successfully logged out from your Anthropic account.\n"
            ),
            Ok(())
        );
    }

    #[test]
    fn a_failed_sign_out_says_why_without_addresses() {
        let err = logout_outcome(
            true,
            false,
            "Logout failed: could not reach https://platform.claude.com/v1/oauth/revoke in time\n",
        )
        .unwrap_err();
        assert_eq!(
            err,
            "Claude Code could not sign out: could not reach in time"
        );
        assert_eq!(
            logout_outcome(true, false, "").unwrap_err(),
            "Claude Code could not sign out."
        );
    }

    /// No answer is not a sign-out, whatever was printed so far.
    #[test]
    fn a_sign_out_that_did_not_finish_did_not_happen() {
        assert!(logout_outcome(false, false, "Successfully logged out").is_err());
    }

    #[test]
    fn every_link_is_https_and_fixed() {
        for link in [
            AccountLink::InstallGuide,
            AccountLink::UpdateGuide,
            AccountLink::Changelog,
        ] {
            assert!(link_url(link).starts_with("https://"), "{link:?}");
        }
    }
}
