#![allow(clippy::unwrap_used)]
//! Step 3 of tech.md 6.4, driven by the body a live response actually carried.
//! The values here are the ones recorded in core v18.

use peekle_core::time::iso_seconds;
use peekle_core::types::{UsageSource, UsageUnavailable, UsageWindow};
use peekle_usage::account::snapshot_from;
use serde_json::{json, Value};

/// Exactly what the probe saw, including the fields we deliberately ignore.
fn live() -> Value {
    json!({
        "five_hour": {
            "utilization": 92.0,
            "resets_at": "2026-08-20T16:30:00.366734+00:00",
            "limit_dollars": null,
            "used_dollars": null,
            "remaining_dollars": null
        },
        "seven_day": {
            "utilization": 10.0,
            "resets_at": "2026-08-26T15:00:00.366760+00:00",
            "limit_dollars": null
        },
        "seven_day_oauth_apps": null,
        "seven_day_opus": null,
        "seven_day_sonnet": null,
        "extra_usage": { "is_enabled": true, "used_credits": 1771.0, "utilization": null },
        "limits": [
            { "kind": "session", "group": "session", "percent": 92, "severity": "critical",
              "resets_at": "2026-08-20T16:30:00.366734+00:00", "is_active": true },
            { "kind": "weekly_all", "group": "weekly", "percent": 10, "severity": "ok",
              "resets_at": "2026-08-26T15:00:00.366760+00:00", "is_active": true }
        ]
    })
}

#[test]
fn the_live_body_gives_both_windows_and_calls_them_by_name() {
    let snapshot = snapshot_from(&live(), 1000);

    assert_eq!(snapshot.source, UsageSource::Account);
    assert_eq!(snapshot.reason, None);
    assert_eq!(snapshot.fetched_at, 1000);
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.windows[0].window, UsageWindow::FiveHour);
    assert_eq!(snapshot.windows[1].window, UsageWindow::SevenDay);
}

/// The one number that has to survive the move off the headers. There it was a
/// fraction, here it is a percentage, and reading it as a fraction would draw
/// 0.92% for a window that is nearly full.
#[test]
fn utilization_is_a_percentage_and_is_not_scaled_again() {
    let snapshot = snapshot_from(&live(), 0);

    assert_eq!(snapshot.windows[0].used_pct, 92.0);
    assert_eq!(snapshot.windows[1].used_pct, 10.0);
}

#[test]
fn the_reset_is_read_as_unix_seconds() {
    let snapshot = snapshot_from(&live(), 0);

    // 2026-08-20T16:30:00Z and 2026-08-26T15:00:00Z.
    assert_eq!(snapshot.windows[0].resets_at, Some(1_787_243_400));
    assert_eq!(snapshot.windows[1].resets_at, Some(1_787_756_400));
}

/// R-3: the body is not a documented API. A shape we do not recognise gives
/// dashes, never a number built out of what happened to be there.
#[test]
fn a_body_that_lost_a_window_reads_as_unsupported() {
    for body in [
        json!({}),
        json!({ "five_hour": { "utilization": 92.0 } }),
        json!({ "seven_day": { "utilization": 10.0 } }),
        json!({ "five_hour": null, "seven_day": { "utilization": 10.0 } }),
        json!({ "five_hour": { "utilization": null }, "seven_day": { "utilization": 10.0 } }),
        json!({ "five_hour": { "used": 92.0 }, "seven_day": { "utilization": 10.0 } }),
    ] {
        let snapshot = snapshot_from(&body, 0);
        assert_eq!(snapshot.source, UsageSource::Unavailable, "{body}");
        assert_eq!(
            snapshot.reason,
            Some(UsageUnavailable::Unsupported),
            "{body}"
        );
        assert!(snapshot.windows.is_empty(), "{body}");
    }
}

/// A window with no reset still draws. Only the countdown goes missing.
#[test]
fn a_window_without_a_reset_still_reports_its_number() {
    let body = json!({
        "five_hour": { "utilization": 5.0, "resets_at": null },
        "seven_day": { "utilization": 6.0 }
    });
    let snapshot = snapshot_from(&body, 0);

    assert_eq!(snapshot.source, UsageSource::Account);
    assert_eq!(snapshot.windows[0].used_pct, 5.0);
    assert_eq!(snapshot.windows[0].resets_at, None);
    assert_eq!(snapshot.windows[1].resets_at, None);
}

#[test]
fn a_number_out_of_range_is_clamped_rather_than_drawn() {
    let body = json!({
        "five_hour": { "utilization": 140.0 },
        "seven_day": { "utilization": -3.0 }
    });
    let snapshot = snapshot_from(&body, 0);

    assert_eq!(snapshot.windows[0].used_pct, 100.0);
    assert_eq!(snapshot.windows[1].used_pct, 0.0);
}

#[test]
fn timestamps_are_read_with_their_offset() {
    assert_eq!(iso_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(iso_seconds("1970-01-01T00:00:00+00:00"), Some(0));
    assert_eq!(iso_seconds("1970-01-01T00:00:00"), Some(0));
    assert_eq!(
        iso_seconds("2026-08-20T16:30:00.366734+00:00"),
        Some(1_787_243_400)
    );
    // The same instant written in two other zones.
    assert_eq!(
        iso_seconds("2026-08-20T18:30:00+02:00"),
        Some(1_787_243_400)
    );
    assert_eq!(iso_seconds("2026-08-20T09:30:00-0700"), Some(1_787_243_400));
    // A leap day, because the civil date arithmetic is hand rolled.
    assert_eq!(iso_seconds("2024-02-29T00:00:00Z"), Some(1_709_164_800));
}

#[test]
fn a_timestamp_that_is_not_one_yields_nothing() {
    for raw in [
        "",
        "tomorrow",
        "2026-08-20",
        "2026-13-20T00:00:00Z",
        "2026-08-32T00:00:00Z",
        "2026-08-20T25:00:00Z",
        "2026-08-20T00:61:00Z",
        "20260820T000000Z",
    ] {
        assert_eq!(iso_seconds(raw), None, "{raw}");
    }
}
