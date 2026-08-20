//! Usage providers. tech.md sections 6.4 and 7.
//!
//! `Unavailable` is a normal state, not an error: the bars render as dashes
//! with the reason spelled out and the panel keeps working. Usage never sits
//! on the critical path of answering a hook.

pub mod account;
pub mod credentials;
pub mod fake;

pub use account::AccountUsage;
pub use credentials::{CredentialError, CredentialStore, FakeCredentialStore, SecurityToolStore};
pub use fake::FakeUsage;

use peekle_core::types::UsageSnapshot;

pub trait UsageProvider: Send + Sync + 'static {
    fn snapshot(&self) -> UsageSnapshot;
}
