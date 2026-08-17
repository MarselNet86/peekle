//! The hook events Peekle manages in `~/.claude/settings.json`.
//! One constant, so `init`, `uninstall` and `doctor` can never drift apart.
//! tech.md section 6.1.

/// Claude Code event names Peekle writes handlers for.
pub const MANAGED_HOOK_EVENTS: &[&str] = &[
    "Stop",
    "PermissionRequest",
    "Notification",
    "PostToolUse",
    "TaskCreated",
    "TaskCompleted",
    "SessionStart",
    "SessionEnd",
];

/// Path prefix every Peekle handler URL carries. `uninstall` and `doctor`
/// identify Peekle's own entries by this prefix plus the loopback host.
pub const HOOK_PATH_PREFIX: &str = "/v1/h/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_events_are_unique() {
        let mut sorted = MANAGED_HOOK_EVENTS.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), before);
    }
}
