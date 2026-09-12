//! Core types, pending registry, config, label classifier.
//! tech.md sections 6.1, 6.3 and 6.8 are the source of truth for this crate.

pub mod agent;
pub mod auth;
pub mod config;
pub mod feed;
pub mod hooks;
pub mod inbox;
pub mod island;
pub mod labels;
pub mod pending;
pub mod pty;
pub mod registry;
pub mod sessions;
pub mod shots;
pub mod time;
pub mod transcripts;
pub mod types;

pub use feed::{truncate, FixtureFeed, TaskFeed, LAST_MESSAGE_LIMIT};
pub use hooks::{HOOK_PATH_PREFIX, MANAGED_HOOK_EVENTS};
pub use island::{shape_rect, Rect};
pub use labels::classify;
pub use pending::PendingRegistry;
pub use pty::{PtyError, PtyHost, SharedPtyHost, SpawnSpec};
pub use sessions::{FeedEvent, SessionRegistry};
pub use shots::{OfferSlot, Pasteboard};
pub use types::*;

/// Where `claude` might be, in the order tech.md 6.4 gives. Relative
/// entries are under the home directory.
#[cfg(not(windows))]
const CLI_CANDIDATES: &[&str] = &[
    ".local/bin/claude",
    "/opt/homebrew/bin/claude",
    "/usr/local/bin/claude",
];

/// The native installer and a global npm install. `.cmd` wrappers are not
/// candidates: the pty starts an executable, not a shell. tech.md 6.27.
#[cfg(windows)]
const CLI_CANDIDATES: &[&str] = &[".local/bin/claude.exe", "AppData/Roaming/npm/claude.exe"];

/// The home directory, under the name each platform keeps it.
///
/// The one place the core asks where the user lives. `HOME` is a unix name and
/// Windows does not set it, so everything that went looking for `~/.claude` by
/// that name alone -- the transcripts, the session registry, Claude Code's own
/// settings, the hook script -- found nothing there and said so by showing an
/// empty list. tech.md 6.27.
pub fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// The first candidate that exists.
///
/// Looked up rather than taken from `PATH`: the app is launched by Finder, and
/// a GUI process inherits a login environment that rarely has the shell's PATH
/// in it. tech.md 6.4.
pub fn claude_path() -> Option<std::path::PathBuf> {
    let home = home_dir()?;
    CLI_CANDIDATES
        .iter()
        .map(|candidate| {
            if candidate.starts_with('/') {
                std::path::PathBuf::from(candidate)
            } else {
                home.join(candidate)
            }
        })
        .find(|path| path.exists())
}
