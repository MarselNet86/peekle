//! Core types, pending registry, config, label classifier.
//! tech.md sections 6.1, 6.3 and 6.8 are the source of truth for this crate.

pub mod config;
pub mod feed;
pub mod hooks;
pub mod island;
pub mod labels;
pub mod pending;
pub mod sessions;
pub mod time;
pub mod tmux;
pub mod transcripts;
pub mod types;

pub use feed::{FixtureFeed, TaskFeed};
pub use hooks::{HOOK_PATH_PREFIX, MANAGED_HOOK_EVENTS};
pub use island::{shape_rect, Rect};
pub use labels::classify;
pub use pending::PendingRegistry;
pub use sessions::{FeedEvent, SessionRegistry};
pub use tmux::{Pane, Tmux};
pub use types::*;

/// Where `claude` might be, in the order tech.md 6.4 gives.
const CLI_CANDIDATES: &[&str] = &[
    ".local/bin/claude",
    "/opt/homebrew/bin/claude",
    "/usr/local/bin/claude",
];

/// The first candidate that exists.
///
/// Looked up rather than taken from `PATH`: the app is launched by Finder, and
/// a GUI process inherits a login environment that rarely has the shell's PATH
/// in it. tech.md 6.4.
pub fn claude_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    CLI_CANDIDATES
        .iter()
        .map(|candidate| {
            if candidate.starts_with('/') {
                std::path::PathBuf::from(candidate)
            } else {
                std::path::Path::new(&home).join(candidate)
            }
        })
        .find(|path| path.exists())
}
