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
pub fn timeout_gap(prompt_timeout_secs: u32, hook_timeout_secs: u32) -> Check {
    if prompt_timeout_secs < hook_timeout_secs {
        return Check::ok(
            "timeout gap",
            format!("{prompt_timeout_secs}s answer window inside a {hook_timeout_secs}s hook"),
        );
    }
    Check::fail(
        "timeout gap",
        format!("{prompt_timeout_secs}s answer window in a {hook_timeout_secs}s hook"),
        "lower behavior.prompt_timeout_secs below the hook timeout",
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

    #[test]
    fn the_gap_is_a_failure_when_peekle_would_answer_too_late() {
        assert_eq!(timeout_gap(600, 900).health, Health::Ok);
        assert_eq!(timeout_gap(900, 900).health, Health::Fail);
        assert_eq!(timeout_gap(1200, 900).health, Health::Fail);
    }

    #[test]
    fn the_gap_failure_says_what_to_change() {
        let check = timeout_gap(900, 900);
        assert!(check.fix.contains("prompt_timeout_secs"));
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
