//! What `peekle doctor` checks, as data rather than as printing. tech.md S9.
//!
//! Every check names the fix. A diagnostic that only says something is wrong
//! sends the user back to the issue tracker.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub health: Health,
    pub detail: String,
    /// What to do about it. Empty when there is nothing to do.
    pub fix: String,
}

impl Check {
    pub fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            health: Health::Ok,
            detail: detail.into(),
            fix: String::new(),
        }
    }

    pub fn warn(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            name,
            health: Health::Warn,
            detail: detail.into(),
            fix: fix.into(),
        }
    }

    pub fn fail(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            name,
            health: Health::Fail,
            detail: detail.into(),
            fix: fix.into(),
        }
    }
}

/// The worst health across the checks, which is what the exit code follows.
pub fn overall(checks: &[Check]) -> Health {
    if checks.iter().any(|c| c.health == Health::Fail) {
        Health::Fail
    } else if checks.iter().any(|c| c.health == Health::Warn) {
        Health::Warn
    } else {
        Health::Ok
    }
}

/// Peekle answers a blocking hook well before Claude Code gives up on it. The
/// gap is the whole reason a slow answer still ends the turn cleanly, so it is
/// checked rather than assumed. tech.md 6.1 and R-5.
/// The shortest window a person can actually use.
///
/// Noticing the island, reading it and answering does not happen in half a
/// minute. A permission window that closes first hands the question back to
/// the terminal, which is safe but reads as the island ignoring it. tech.md R-5.
const USABLE_WINDOW_SECS: u32 = 60;

/// The permission window against the hook timeout. Only permission: a Stop
/// that gives up costs nothing, because the text it was waiting for stays
/// queued for the next one. tech.md 6.8.
pub fn timeout_gap(permission_wait_secs: u32, hook_timeout_secs: u32) -> Check {
    if permission_wait_secs >= hook_timeout_secs {
        return Check::fail(
            "timeout gap",
            format!("{permission_wait_secs}s permission window in a {hook_timeout_secs}s hook"),
            "lower behavior.permission_wait_secs below the hook timeout",
        );
    }
    if permission_wait_secs < USABLE_WINDOW_SECS {
        return Check::warn(
            "timeout gap",
            format!("{permission_wait_secs}s permission window, too short to answer in"),
            "raise behavior.permission_wait_secs: the question goes back to the terminal",
        );
    }
    Check::ok(
        "timeout gap",
        format!("{permission_wait_secs}s permission window inside a {hook_timeout_secs}s hook"),
    )
}

/// Claude Code caps how many times a Stop hook may block in a row. Peekle
/// blocks on every answer, so a long session runs into it. tech.md R-2.
pub fn stop_block_cap(raw: Option<&str>) -> Check {
    match raw.map(str::trim) {
        Some("0") => Check::ok("stop block cap", "lifted"),
        Some(value) if !value.is_empty() => Check::warn(
            "stop block cap",
            format!("CLAUDE_CODE_STOP_HOOK_BLOCK_CAP is {value}"),
            "set it to 0 so a long session keeps answering from the island",
        ),
        _ => Check::warn(
            "stop block cap",
            "unset, so Claude Code uses its default",
            "export CLAUDE_CODE_STOP_HOOK_BLOCK_CAP=0 to keep answering past it",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window that made the product look broken: eleven turns ended
    /// normally because thirty seconds is not long enough to answer in.
    #[test]
    fn a_window_nobody_can_answer_in_is_a_warning() {
        assert_eq!(timeout_gap(30, 900).health, Health::Warn);
        assert_eq!(timeout_gap(59, 900).health, Health::Warn);
        assert_eq!(timeout_gap(60, 900).health, Health::Ok);
        assert!(timeout_gap(30, 900).detail.contains("too short"));
    }

    #[test]
    fn the_gap_is_a_failure_when_peekle_would_answer_too_late() {
        assert_eq!(timeout_gap(600, 900).health, Health::Ok);
        assert_eq!(timeout_gap(900, 900).health, Health::Fail);
        assert_eq!(timeout_gap(1200, 900).health, Health::Fail);
    }

    #[test]
    fn the_gap_failure_says_what_to_change() {
        let check = timeout_gap(900, 900);
        assert!(check.fix.contains("permission_wait_secs"));
    }

    #[test]
    fn only_a_lifted_cap_is_clean() {
        assert_eq!(stop_block_cap(Some("0")).health, Health::Ok);
        assert_eq!(stop_block_cap(Some("8")).health, Health::Warn);
        assert_eq!(stop_block_cap(None).health, Health::Warn);
        assert_eq!(stop_block_cap(Some("  ")).health, Health::Warn);
    }

    #[test]
    fn overall_follows_the_worst_check() {
        assert_eq!(overall(&[]), Health::Ok);
        assert_eq!(overall(&[Check::ok("a", "")]), Health::Ok);
        assert_eq!(
            overall(&[Check::ok("a", ""), Check::warn("b", "", "")]),
            Health::Warn
        );
        assert_eq!(
            overall(&[Check::warn("b", "", ""), Check::fail("c", "", "")]),
            Health::Fail
        );
    }
}
