//! Usage providers. tech.md sections 6.4 and 7.
//!
//! `Unavailable` is a normal state, not an error: the bars render as dashes
//! with the reason spelled out and the panel keeps working. Usage never sits
//! on the critical path of answering a hook.

pub mod account;
pub mod credentials;
pub mod fake;

pub use account::{AccountUsage, TIMEOUT as POLL_TIMEOUT};
pub use credentials::{CredentialError, CredentialStore, FakeCredentialStore, SecurityToolStore};
pub use fake::FakeUsage;

use peekle_core::types::UsageSnapshot;

pub trait UsageProvider: Send + Sync + 'static {
    fn snapshot(&self) -> UsageSnapshot;

    /// A snapshot bounded by what the caller can wait for. Only the account
    /// provider has anything to bound; a fake answers instantly. tech.md 6.4.
    fn snapshot_within(&self, _budget: std::time::Duration) -> UsageSnapshot {
        self.snapshot()
    }
}

/// How long the background poll waits after a network failure. tech.md 6.4.
///
/// A ladder rather than a flat wait: an interface comes back before its route
/// and its DNS do, so the first attempts are cheap and close together, and a
/// machine with no network at all settles at one attempt every half minute
/// rather than hammering the endpoint. The first success resets the count, so
/// the ladder is climbed from the bottom every time the connection drops.
pub fn retry_after(misses: u32) -> std::time::Duration {
    const LADDER: [u64; 5] = [2, 5, 10, 20, 30];
    let index = misses.max(1) as usize - 1;
    let seconds = LADDER[index.min(LADDER.len() - 1)];
    std::time::Duration::from_secs(seconds)
}

#[cfg(test)]
mod tests {
    use super::retry_after;
    use std::time::Duration;

    /// A returning network is caught in seconds, not in half a minute.
    #[test]
    fn the_first_retry_comes_quickly() {
        assert_eq!(retry_after(1), Duration::from_secs(2));
        assert_eq!(retry_after(2), Duration::from_secs(5));
    }

    /// Four attempts inside the first forty seconds, so the wifi that came
    /// back while the laptop was closed is noticed on opening it.
    #[test]
    fn the_early_attempts_are_close_together() {
        let first_four: u64 = (1..=4).map(|n| retry_after(n).as_secs()).sum();
        assert!(first_four <= 40, "{first_four}s across four attempts");
    }

    #[test]
    fn it_never_settles_below_or_above_the_ladder() {
        for misses in [5, 6, 50, u32::MAX] {
            assert_eq!(retry_after(misses), Duration::from_secs(30), "{misses}");
        }
        // A zero count is the same as the first miss: there is no attempt
        // number zero, and a zero wait would be a spin.
        assert_eq!(retry_after(0), retry_after(1));
    }

    #[test]
    fn the_ladder_only_grows() {
        let waits: Vec<u64> = (1..=8).map(|n| retry_after(n).as_secs()).collect();
        let mut sorted = waits.clone();
        sorted.sort_unstable();
        assert_eq!(waits, sorted);
        assert!(waits.iter().all(|seconds| *seconds > 0));
    }
}
