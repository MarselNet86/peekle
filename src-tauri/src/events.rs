//! Event names emitted to the frontend. tech.md section 6.6, literal.
//! The frontend never emits Tauri events; intents travel as commands only.

pub const PROMPT_OPEN: &str = "peekle://prompt-open";
pub const PROMPT_CLOSE: &str = "peekle://prompt-close";
pub const SESSIONS: &str = "peekle://sessions";
pub const TASKS: &str = "peekle://tasks";
pub const USAGE: &str = "peekle://usage";
pub const ENABLED: &str = "peekle://enabled";
pub const TOAST: &str = "peekle://toast";
pub const VIEW: &str = "peekle://view";
