//! The hook events Peekle manages in `~/.claude/settings.json`.
//! One constant, so `init`, `uninstall` and `doctor` can never drift apart.
//! tech.md section 6.1.

/// Claude Code event names Peekle writes handlers for.
pub const MANAGED_HOOK_EVENTS: &[&str] = &[
    "Stop",
    "PermissionRequest",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "SessionStart",
    "SessionEnd",
];

/// Path prefix every Peekle endpoint carries. The hook script posts to it, and
/// it still identifies handlers written by older versions, which were `http`.
pub const HOOK_PATH_PREFIX: &str = "/v1/h/";

/// The script `init` installs as the handler for every managed event.
///
/// A script rather than an `http` handler because it runs as a child of the
/// agent, and that is the only way to learn the pid and tty a session lives
/// on. Nothing else can: an HTTP handler arrives over a socket knowing nothing
/// about the process that triggered it. tech.md 6.1.
pub const HOOK_SCRIPT_NAME: &str = "peekle-hook.py";

/// Where the script lives, under the user's Claude directory.
pub fn hook_script_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::Path::new(&home)
        .join(".claude")
        .join("peekle")
        .join(HOOK_SCRIPT_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The feed needs all three or the row never closes. tech.md 6.1 and 6.3.
    #[test]
    fn the_feed_events_are_all_managed() {
        for event in ["UserPromptSubmit", "PreToolUse", "PostToolUse"] {
            assert!(MANAGED_HOOK_EVENTS.contains(&event), "{event} is unmanaged");
        }
    }

    #[test]
    fn managed_events_are_unique() {
        let mut sorted = MANAGED_HOOK_EVENTS.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), before);
    }
}
