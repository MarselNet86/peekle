//! Pending blocking hook requests.
//!
//! The one invariant this file exists to hold: every registered request is
//! resolved exactly once, on every path, including timeout, bypass and error.
//! A leaked pending request hangs a live agent run until its hook times out.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::oneshot;

use crate::types::PromptOutcome;

#[derive(Debug, Default)]
pub struct PendingRegistry {
    inner: Mutex<HashMap<String, oneshot::Sender<PromptOutcome>>>,
}

impl PendingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a request and hands back the channel the hook handler awaits.
    /// Registering an id that is already pending replaces the old sender and
    /// resolves it as `Dismissed`, so no waiter is ever left without an answer.
    pub fn register(&self, id: impl Into<String>) -> oneshot::Receiver<PromptOutcome> {
        let (tx, rx) = oneshot::channel();
        let replaced = {
            let mut map = self.lock();
            map.insert(id.into(), tx)
        };
        if let Some(old) = replaced {
            let _ = old.send(PromptOutcome::Dismissed);
        }
        rx
    }

    /// Resolves a pending request. Returns true when this call is the one that
    /// resolved it, false when the id is unknown or already resolved. A second
    /// answer for the same id is a no-op, not an error.
    pub fn resolve(&self, id: &str, outcome: PromptOutcome) -> bool {
        let sender = {
            let mut map = self.lock();
            map.remove(id)
        };
        match sender {
            // The receiver being gone still counts: the request is settled and
            // will never be answered twice.
            Some(tx) => {
                let _ = tx.send(outcome);
                true
            }
            None => false,
        }
    }

    /// Resolves every pending request with the same outcome. Used when the
    /// global toggle goes off and when the app shuts down.
    pub fn resolve_all(&self, outcome: PromptOutcome) -> usize {
        let drained: Vec<_> = {
            let mut map = self.lock();
            map.drain().map(|(_, tx)| tx).collect()
        };
        let count = drained.len();
        for tx in drained {
            let _ = tx.send(outcome.clone());
        }
        count
    }

    pub fn is_pending(&self, id: &str) -> bool {
        self.lock().contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A poisoned lock means a previous holder panicked mid-update. The map is
    /// still structurally sound, and refusing to answer would hang the agent,
    /// so recover instead of propagating.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, oneshot::Sender<PromptOutcome>>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PromptAnswer;

    fn answer(id: &str) -> PromptOutcome {
        PromptOutcome::Answered(PromptAnswer {
            prompt_id: id.to_string(),
            choice: None,
            text: Some("go on".to_string()),
        })
    }

    #[tokio::test]
    async fn resolves_once_and_delivers_the_outcome() {
        let registry = PendingRegistry::new();
        let rx = registry.register("a");

        assert!(registry.resolve("a", answer("a")));
        assert_eq!(rx.await.unwrap(), answer("a"));
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn second_resolve_is_a_no_op() {
        let registry = PendingRegistry::new();
        let rx = registry.register("a");

        assert!(registry.resolve("a", answer("a")));
        assert!(!registry.resolve("a", PromptOutcome::Dismissed));
        assert_eq!(rx.await.unwrap(), answer("a"));
    }

    #[test]
    fn resolving_an_unknown_id_is_a_no_op() {
        let registry = PendingRegistry::new();
        assert!(!registry.resolve("nobody", PromptOutcome::Dismissed));
    }

    #[tokio::test]
    async fn resolve_after_the_waiter_left_does_not_panic() {
        let registry = PendingRegistry::new();
        let rx = registry.register("a");
        drop(rx);

        assert!(registry.resolve("a", PromptOutcome::TimedOut));
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn re_registering_the_same_id_settles_the_old_waiter() {
        let registry = PendingRegistry::new();
        let first = registry.register("a");
        let second = registry.register("a");

        assert_eq!(first.await.unwrap(), PromptOutcome::Dismissed);
        assert!(registry.resolve("a", PromptOutcome::Bypassed));
        assert_eq!(second.await.unwrap(), PromptOutcome::Bypassed);
    }

    #[tokio::test]
    async fn resolve_all_settles_every_waiter() {
        let registry = PendingRegistry::new();
        let a = registry.register("a");
        let b = registry.register("b");

        assert_eq!(registry.resolve_all(PromptOutcome::Bypassed), 2);
        assert_eq!(a.await.unwrap(), PromptOutcome::Bypassed);
        assert_eq!(b.await.unwrap(), PromptOutcome::Bypassed);
        assert!(registry.is_empty());
    }
}
