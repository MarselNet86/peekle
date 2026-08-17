//! Property based tests for the pure logic of the core crate.
//! tech.md section 10 names these: usage window math and the label classifier.

use peekle_core::labels::classify;
use peekle_core::types::{clamp_pct, UsageWindow, UsageWindowStat};
use proptest::prelude::*;

proptest! {
    /// The classifier feeds the HUD directly. A panic there takes down a
    /// window the user cannot reopen, so it has to be total over any title.
    #[test]
    fn classify_is_total(title in ".*") {
        let _ = classify(&title);
    }

    #[test]
    fn classify_is_stable(title in ".*") {
        prop_assert_eq!(classify(&title), classify(&title));
    }

    /// Case folding must not change the verdict, otherwise a title typed in
    /// caps lands in a different lane than the same title in lower case.
    #[test]
    fn classify_ignores_case(title in "[a-zA-Z ]{0,64}") {
        prop_assert_eq!(classify(&title), classify(&title.to_uppercase()));
    }

    /// Rate limit headers are undocumented. Whatever number arrives, the bar
    /// has to render.
    #[test]
    fn clamp_pct_lands_in_range(pct in proptest::num::f32::ANY) {
        let clamped = clamp_pct(pct);
        prop_assert!((0.0..=100.0).contains(&clamped));
        prop_assert!(!clamped.is_nan());
    }

    #[test]
    fn clamp_pct_is_idempotent(pct in proptest::num::f32::ANY) {
        prop_assert_eq!(clamp_pct(clamp_pct(pct)), clamp_pct(pct));
    }

    #[test]
    fn clamp_pct_is_monotone(a in -1.0e6f32..1.0e6, b in -1.0e6f32..1.0e6) {
        prop_assume!(a <= b);
        prop_assert!(clamp_pct(a) <= clamp_pct(b));
    }

    /// A utilization fraction of 0..1 scaled by 100 always yields a usable bar.
    #[test]
    fn window_stat_accepts_any_utilization(fraction in proptest::num::f32::ANY) {
        let stat = UsageWindowStat::new(UsageWindow::FiveHour, fraction * 100.0, None);
        prop_assert!((0.0..=100.0).contains(&stat.used_pct));
    }
}
