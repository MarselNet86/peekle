//! Loopback hook server. tech.md sections 6.1 and 6.2 hold the contract.
//!
//! Two rules run through every handler here. A blocking endpoint resolves its
//! pending request exactly once on every path, and no path returns a status
//! outside 2xx except the documented 404 and 400: for Claude Code a 5xx is a
//! non-blocking error, and an empty decision is the safe fallback anyway.

pub mod map;
pub mod routes;
pub mod sink;

pub use routes::{router, CORE_VERSION};
pub use sink::{HookSink, StopPlan};
