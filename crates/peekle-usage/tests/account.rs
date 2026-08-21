#![allow(clippy::unwrap_used)]
//! Step 3 of tech.md 6.4, driven by the headers a live response actually
//! carried. The values here are the ones recorded in core v13.

use std::collections::HashMap;

use peekle_core::types::{UsageSource, UsageUnavailable, UsageWindow};
use peekle_usage::account::{snapshot_from, Headers};

struct Map(HashMap<String, String>);

impl Headers for Map {
    fn get(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

/// Exactly what the probe saw, including the headers we deliberately ignore.
fn live() -> Map {
    Map(HashMap::from([
        (
            "anthropic-ratelimit-unified-status".into(),
            "allowed".into(),
        ),
        (
            "anthropic-ratelimit-unified-5h-status".into(),
            "allowed".into(),
        ),
        (
            "anthropic-ratelimit-unified-5h-reset".into(),
            "1787140200".into(),
        ),
        (
            "anthropic-ratelimit-unified-5h-utilization".into(),
            "0.17".into(),
        ),
        (
            "anthropic-ratelimit-unified-7d-status".into(),
            "allowed".into(),
        ),
        (
            "anthropic-ratelimit-unified-7d-reset".into(),
            "1787151600".into(),
        ),
        (
            "anthropic-ratelimit-unified-7d-utilization".into(),
            "0.48".into(),
        ),
        (
            "anthropic-ratelimit-unified-overage-utilization".into(),
            "0.0".into(),
        ),
        (
            "anthropic-ratelimit-unified-representative-claim".into(),
            "five_hour".into(),
        ),
        (
            "anthropic-ratelimit-unified-fallback-percentage".into(),
            "0.5".into(),
        ),
    ]))
}

fn without(name: &str) -> Map {
    let mut map = live();
    map.0.remove(name);
    map
}

#[test]
fn a_live_response_becomes_two_windows() {
    let snapshot = snapshot_from(&live(), 1000);

    assert_eq!(snapshot.source, UsageSource::Account);
    assert!(snapshot.reason.is_none());
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.fetched_at, 1000);
}

/// The fraction question the contract told us to settle live: 0.17 is
/// seventeen percent, not seventeen hundredths of a percent.
#[test]
fn utilization_is_a_fraction_of_one() {
    let snapshot = snapshot_from(&live(), 0);

    let five = snapshot
        .windows
        .iter()
        .find(|w| w.window == UsageWindow::FiveHour)
        .unwrap();
    let week = snapshot
        .windows
        .iter()
        .find(|w| w.window == UsageWindow::SevenDay)
        .unwrap();

    assert!((five.used_pct - 17.0).abs() < 0.01, "{}", five.used_pct);
    assert!((week.used_pct - 48.0).abs() < 0.01, "{}", week.used_pct);
    assert_eq!(five.resets_at, Some(1787140200));
    assert_eq!(week.resets_at, Some(1787151600));
}

/// R-3. The headers are undocumented and can change or vanish. When they do,
/// the bars go to dashes rather than to a number assembled from leftovers.
#[test]
fn any_missing_header_of_the_four_means_unsupported() {
    for name in [
        "anthropic-ratelimit-unified-5h-utilization",
        "anthropic-ratelimit-unified-5h-reset",
        "anthropic-ratelimit-unified-7d-utilization",
        "anthropic-ratelimit-unified-7d-reset",
    ] {
        let snapshot = snapshot_from(&without(name), 0);

        assert_eq!(snapshot.source, UsageSource::Unavailable, "{name}");
        assert_eq!(
            snapshot.reason,
            Some(UsageUnavailable::Unsupported),
            "{name}"
        );
        assert!(snapshot.windows.is_empty(), "{name}");
    }
}

#[test]
fn a_header_that_is_not_a_number_is_unsupported_not_zero() {
    let mut map = live();
    map.0.insert(
        "anthropic-ratelimit-unified-5h-utilization".into(),
        "quite a lot".into(),
    );

    let snapshot = snapshot_from(&map, 0);
    assert_eq!(snapshot.reason, Some(UsageUnavailable::Unsupported));
}

#[test]
fn an_empty_response_is_unsupported() {
    let snapshot = snapshot_from(&Map(HashMap::new()), 0);
    assert_eq!(snapshot.reason, Some(UsageUnavailable::Unsupported));
}

/// The bars read a percentage, so whatever the header says has to land in
/// 0..100 before it reaches them.
#[test]
fn an_out_of_range_fraction_is_clamped_rather_than_shown() {
    for (raw, expected) in [("2.5", 100.0), ("-1", 0.0), ("1", 100.0)] {
        let mut map = live();
        map.0.insert(
            "anthropic-ratelimit-unified-5h-utilization".into(),
            raw.into(),
        );

        let snapshot = snapshot_from(&map, 0);
        let five = snapshot
            .windows
            .iter()
            .find(|w| w.window == UsageWindow::FiveHour)
            .unwrap();
        assert!(
            (five.used_pct - expected).abs() < 0.01,
            "{raw} became {}",
            five.used_pct
        );
    }
}

/// Whitespace around a header value is the server's business, not ours.
#[test]
fn values_survive_being_padded() {
    let mut map = live();
    map.0.insert(
        "anthropic-ratelimit-unified-7d-utilization".into(),
        "  0.48  ".into(),
    );
    let snapshot = snapshot_from(&map, 0);
    assert_eq!(snapshot.source, UsageSource::Account);
}
