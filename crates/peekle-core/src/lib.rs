//! Core types, pending registry, config, label classifier.
//! tech.md sections 6.1, 6.3 and 6.8 are the source of truth for this crate.

pub mod account;
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

/// The home directory, under the name each platform keeps it. tech.md 6.27.
fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// The first candidate that exists, and failing that the first `claude` on
/// the login shell's `PATH`.
///
/// Looked up rather than taken from this process's `PATH`: the app is launched
/// by Finder, and a GUI process inherits a login environment that rarely has
/// the shell's PATH in it. The known places alone were not enough either: a
/// `claude` under nvm, or in a directory of the person's own, read as "not
/// installed" to someone who has it. tech.md 6.4 and 6.16.
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
        .or_else(|| {
            let path = crate::pty::session_path();
            find_on_path(std::env::split_paths(&path), CLI_NAME)
        })
}

#[cfg(not(windows))]
const CLI_NAME: &str = "claude";
#[cfg(windows)]
const CLI_NAME: &str = "claude.exe";

/// The first file called `name` in `dirs`, in order. tech.md 6.16.
pub fn find_on_path<I>(dirs: I, name: &str) -> Option<std::path::PathBuf>
where
    I: IntoIterator<Item = std::path::PathBuf>,
{
    dirs.into_iter()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}
