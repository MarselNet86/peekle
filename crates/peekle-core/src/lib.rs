//! Core types, pending registry, config, label classifier.
//! tech.md sections 6.1, 6.3 and 6.8 are the source of truth for this crate.

pub mod config;
pub mod hooks;
pub mod labels;
pub mod pending;
pub mod types;

pub use hooks::{HOOK_PATH_PREFIX, MANAGED_HOOK_EVENTS};
pub use labels::classify;
pub use pending::PendingRegistry;
pub use types::*;
