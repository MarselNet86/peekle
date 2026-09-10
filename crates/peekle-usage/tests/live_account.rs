#![allow(clippy::unwrap_used)]
//! The one test that talks to the network and to the Keychain.
//!
//! Ignored by default: it can raise a Keychain dialog, and CI has neither an
//! account nor a Keychain. It costs nothing to run otherwise, the endpoint
//! only reads. Run it by hand after touching anything in 6.4, which is the
//! only way to find out that the undocumented body of R-3 has changed.
//!
//!     cargo test -p peekle-usage --test live_account -- --ignored --nocapture

use peekle_core::types::{UsageSource, UsageWindow};
use peekle_usage::{AccountUsage, SecurityToolStore, UsageProvider};

#[test]
#[ignore = "reads the live account with the user's own token"]
fn the_live_account_still_reports_both_windows() {
    let provider = AccountUsage::new(Box::new(SecurityToolStore::for_current_user()), "0.1.0");
    let snapshot = provider.snapshot();

    println!(
        "source: {:?}, reason: {:?}",
        snapshot.source, snapshot.reason
    );
    for stat in &snapshot.windows {
        println!(
            "{:?}: {:.1}% resets at {:?}",
            stat.window, stat.used_pct, stat.resets_at
        );
    }

    assert_eq!(
        snapshot.source,
        UsageSource::Account,
        "the body of 6.4 stopped arriving, so R-3 came true: {:?}",
        snapshot.reason
    );
    assert_eq!(snapshot.windows.len(), 2);

    for window in [UsageWindow::FiveHour, UsageWindow::SevenDay] {
        let stat = snapshot
            .windows
            .iter()
            .find(|s| s.window == window)
            .unwrap_or_else(|| panic!("{window:?} missing"));

        assert!((0.0..=100.0).contains(&stat.used_pct), "{stat:?}");
        assert!(stat.resets_at.is_some_and(|at| at > 0), "{stat:?}");
    }
}
