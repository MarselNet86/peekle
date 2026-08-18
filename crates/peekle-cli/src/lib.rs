//! `peekle` command line. tech.md S9.
//!
//! Everything here touches the user's own files, so every write is merged,
//! idempotent and backed up first. `~/.claude/settings.json` is the user's
//! territory and this crate is the only thing allowed to write it.
//! tech.md section 11.

pub mod doctor;
pub mod settings;

pub use settings::{
    is_peekle_handler, merge_handlers, remove_handlers, ClaudeSettings, FakeSettings, FileSettings,
    SettingsError,
};
