//! Fake usage provider. Default in dev so a session never waits on a real
//! Keychain prompt or a real network round trip. tech.md section 7.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use peekle_core::types::{
    UsageSnapshot, UsageSource, UsageUnavailable, UsageWindow, UsageWindowStat,
};

use crate::UsageProvider;

/// The model a fake plan counts on its own, standing in for whatever the real
/// endpoint names in `scope.model.display_name`. A stand-in, not a claim: the
/// real provider never invents this word. tech.md 6.4 and section 7.
const SCOPED_MODEL: &str = "Fable";

/// Every mode the real provider can end up in, so the UI for each one is
/// reachable without breaking anything on purpose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FakeMode {
    /// Two windows with fixed percentages.
    Numbers { five_hour: f32, seven_day: f32 },
    /// One of the documented unavailable reasons.
    Unavailable(UsageUnavailable),
}

impl Default for FakeMode {
    fn default() -> Self {
        FakeMode::Numbers {
            five_hour: 42.0,
            seven_day: 68.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct FakeUsage {
    mode: Mutex<FakeMode>,
}

impl FakeUsage {
    pub fn new(mode: FakeMode) -> Self {
        Self {
            mode: Mutex::new(mode),
        }
    }

    pub fn set_mode(&self, mode: FakeMode) {
        *self.lock() = mode;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeMode> {
        self.mode.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn now_secs() -> i64 {
    now_ms() / 1000
}

impl UsageProvider for FakeUsage {
    fn snapshot(&self) -> UsageSnapshot {
        match *self.lock() {
            FakeMode::Numbers {
                five_hour,
                seven_day,
            } => UsageSnapshot {
                windows: vec![
                    UsageWindowStat::new(
                        UsageWindow::FiveHour,
                        five_hour,
                        Some(now_secs() + 90 * 60),
                    ),
                    UsageWindowStat::new(
                        UsageWindow::SevenDay,
                        seven_day,
                        Some(now_secs() + 4 * 24 * 60 * 60),
                    ),
                    // A plan that counts one model apart, so the third dial is
                    // reachable in dev. Lower than the whole week, because a
                    // part of it cannot be more than all of it.
                    UsageWindowStat::scoped(
                        seven_day * 0.8,
                        Some(now_secs() + 4 * 24 * 60 * 60),
                        SCOPED_MODEL,
                    ),
                ],
                source: UsageSource::Fake,
                reason: None,
                fetched_at: now_ms(),
                // Stamped centrally, same as the real provider. tech.md 6.4.
                keychain_granted: false,
                retry_after_ms: None,
            },
            FakeMode::Unavailable(reason) => UsageSnapshot {
                windows: Vec::new(),
                source: UsageSource::Unavailable,
                reason: Some(reason),
                fetched_at: now_ms(),
                keychain_granted: false,
                retry_after_ms: None,
            },
        }
    }
}

/// A snapshot that says nothing is known yet. Used before the first poll so
/// the panel always has something to render.
pub fn unknown(reason: UsageUnavailable) -> UsageSnapshot {
    UsageSnapshot {
        windows: Vec::new(),
        source: UsageSource::Unavailable,
        reason: Some(reason),
        fetched_at: now_ms(),
        keychain_granted: false,
        retry_after_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_mode_yields_both_windows_in_range() {
        let snapshot = FakeUsage::default().snapshot();
        assert_eq!(snapshot.windows.len(), 3);
        assert_eq!(snapshot.source, UsageSource::Fake);
        assert_eq!(snapshot.reason, None);
        for window in &snapshot.windows {
            assert!((0.0..=100.0).contains(&window.used_pct));
            assert!(window.resets_at.is_some());
        }
    }

    /// The third window is the one a plan may not have at all, so it is the
    /// one the fake has to be able to show. It carries a name; the other two
    /// never do. tech.md 6.4.
    #[test]
    fn the_scoped_window_carries_a_name_and_the_others_do_not() {
        let snapshot = FakeUsage::default().snapshot();

        assert_eq!(snapshot.windows[2].window, UsageWindow::SevenDayScoped);
        assert_eq!(snapshot.windows[2].scope.as_deref(), Some(SCOPED_MODEL));
        assert_eq!(snapshot.windows[0].scope, None);
        assert_eq!(snapshot.windows[1].scope, None);
    }

    #[test]
    fn out_of_range_numbers_are_clamped_not_rejected() {
        let fake = FakeUsage::new(FakeMode::Numbers {
            five_hour: -20.0,
            seven_day: 900.0,
        });
        let snapshot = fake.snapshot();
        assert_eq!(snapshot.windows[0].used_pct, 0.0);
        assert_eq!(snapshot.windows[1].used_pct, 100.0);
    }

    /// Every documented reason has to be reachable, otherwise its rendering
    /// path is never exercised before a user hits it.
    #[test]
    fn every_unavailable_reason_is_reachable() {
        for reason in [
            UsageUnavailable::Disabled,
            UsageUnavailable::NotGranted,
            UsageUnavailable::Denied,
            UsageUnavailable::NotLoggedIn,
            UsageUnavailable::Offline,
            UsageUnavailable::Network,
            UsageUnavailable::RateLimited,
            UsageUnavailable::Unsupported,
        ] {
            let snapshot = FakeUsage::new(FakeMode::Unavailable(reason)).snapshot();
            assert!(snapshot.windows.is_empty());
            assert_eq!(snapshot.source, UsageSource::Unavailable);
            assert_eq!(snapshot.reason, Some(reason));
        }
    }

    #[test]
    fn mode_can_be_switched_at_runtime() {
        let fake = FakeUsage::default();
        assert_eq!(fake.snapshot().source, UsageSource::Fake);
        fake.set_mode(FakeMode::Unavailable(UsageUnavailable::Denied));
        assert_eq!(fake.snapshot().source, UsageSource::Unavailable);
    }
}
