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

/// The body as it came back on 2026-09-10, cut to what step 3 reads. The
/// account had grown a third window since core v18: the plan counts one model
/// on its own, and that window has no key of its own -- it is a row of
/// `limits`. tech.md 6.4.
fn live_scoped() -> Value {
    json!({
        "five_hour": {
            "utilization": 5.0,
            "resets_at": "2026-09-10T11:40:00.025122+00:00",
            "locked_reason": null
        },
        "seven_day": {
            "utilization": 27.0,
            "resets_at": "2026-09-14T17:00:00.025152+00:00",
            "locked_reason": null
        },
        "seven_day_oauth_apps": null,
        "seven_day_opus": null,
        "seven_day_sonnet": null,
        "extra_usage": { "is_enabled": false, "used_credits": 0.0, "utilization": 0.0 },
        "limits": [
            { "kind": "session", "group": "session", "percent": 5, "severity": "normal",
              "resets_at": "2026-09-10T11:40:00.025122+00:00", "scope": null, "is_active": false },
            { "kind": "weekly_all", "group": "weekly", "percent": 27, "severity": "normal",
              "resets_at": "2026-09-14T17:00:00.025152+00:00", "scope": null, "is_active": true },
            { "kind": "weekly_scoped", "group": "weekly", "percent": 22, "severity": "normal",
              "resets_at": "2026-09-14T17:00:00.025419+00:00",
              "scope": { "model": { "id": null, "display_name": "Fable" }, "surface": null },
              "is_active": false }
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

/// v62 acceptance. Claude Code checks the clock before every authenticated
/// call and refreshes rather than waiting to be refused; Peekle cannot
/// refresh, so it copies the half it can -- it declines to spend a request on
/// a credential the CLI itself would have replaced first. tech.md 6.4.
mod the_credential_clock {
    use peekle_core::types::UsageUnavailable;
    use peekle_usage::credentials::{access_token, expires_at, FakeCredentialStore};
    use peekle_usage::{AccountUsage, UsageProvider};

    fn entry(expires_at: i64) -> String {
        format!(
            r#"{{"claudeAiOauth":{{"accessToken":"tok","refreshToken":"ref","expiresAt":{expires_at}}}}}"#
        )
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default()
    }

    #[test]
    fn reads_the_expiry_the_cli_writes() {
        assert_eq!(
            expires_at(&entry(1_770_000_000_000)),
            Some(1_770_000_000_000)
        );
        assert_eq!(access_token(&entry(1)).as_deref(), Some("tok"));
    }

    /// An older entry has no expiry at all. That is not a reason to refuse
    /// it: it only means the clock cannot be consulted.
    #[test]
    fn an_entry_without_an_expiry_is_still_used() {
        assert_eq!(
            expires_at(r#"{"claudeAiOauth":{"accessToken":"tok"}}"#),
            None
        );
        assert_eq!(expires_at("not json"), None);
        assert_eq!(expires_at("{}"), None);
    }

    #[test]
    fn a_spent_credential_costs_no_request() {
        let dir = std::env::temp_dir().join(format!("peekle-cred-{}", now_ms()));
        let store = FakeCredentialStore::with_contents(&dir, &entry(now_ms() - 60_000)).unwrap();
        let usage = AccountUsage::new(Box::new(store), "0.1.0");

        // No network is reachable from a unit test, and none is needed: the
        // clock answers before the request would be made.
        let snapshot = usage.snapshot_within(std::time::Duration::from_millis(1));
        assert_eq!(snapshot.reason, Some(UsageUnavailable::NotLoggedIn));
    }
}

/// The window a plan counts for one model on its own. It arrives as a row of
/// `limits`, and its name comes from the server. tech.md 6.4.
#[test]
fn a_plan_that_counts_one_model_apart_gives_a_third_window() {
    let snapshot = snapshot_from(&live_scoped(), 0);

    assert_eq!(snapshot.source, UsageSource::Account);
    assert_eq!(snapshot.windows.len(), 3);

    let scoped = &snapshot.windows[2];
    assert_eq!(scoped.window, UsageWindow::SevenDayScoped);
    assert_eq!(scoped.scope.as_deref(), Some("Fable"));
    assert_eq!(scoped.used_pct, 22.0);
    // 2026-09-14T17:00:00Z.
    assert_eq!(scoped.resets_at, Some(1_789_405_200));
}

/// The two windows every account has are never named: a name means the window
/// belongs to one model, and reading one where there is none would put a
/// label on the whole week.
#[test]
fn the_windows_of_the_account_itself_carry_no_name() {
    let snapshot = snapshot_from(&live_scoped(), 0);

    assert_eq!(snapshot.windows[0].scope, None);
    assert_eq!(snapshot.windows[1].scope, None);
}

/// The plan has no such window, so there is no third dial. A body from before
/// the endpoint reported one reads the same way, which is the point: the two
/// windows are the contract, the third is a bonus.
#[test]
fn a_plan_without_one_gives_two_windows_and_no_dashes() {
    for body in [
        live(),
        json!({
            "five_hour": { "utilization": 5.0 },
            "seven_day": { "utilization": 6.0 }
        }),
        // The weekly row is there but scoped to nothing, which is what
        // `weekly_all` is: the number already arrived as `seven_day`.
        json!({
            "five_hour": { "utilization": 5.0 },
            "seven_day": { "utilization": 6.0 },
            "limits": [{ "kind": "weekly_all", "group": "weekly", "percent": 6, "scope": null }]
        }),
        // Scoped to something that is not a model, or to a model with no name:
        // there is nothing to write under the dial, so there is no dial.
        json!({
            "five_hour": { "utilization": 5.0 },
            "seven_day": { "utilization": 6.0 },
            "limits": [{ "group": "weekly", "percent": 22, "scope": { "surface": "code" } }]
        }),
        json!({
            "five_hour": { "utilization": 5.0 },
            "seven_day": { "utilization": 6.0 },
            "limits": [{ "group": "weekly", "percent": 22, "scope": { "model": { "id": "fable" } } }]
        }),
        // Weekly is the group that is read. A session row scoped to a model
        // is a different window, and the five hour number already arrived.
        json!({
            "five_hour": { "utilization": 5.0 },
            "seven_day": { "utilization": 6.0 },
            "limits": [{ "kind": "session", "group": "session", "percent": 5,
                         "scope": { "model": { "display_name": "Fable" } } }]
        }),
        // Not an array, so there is nothing to walk.
        json!({
            "five_hour": { "utilization": 5.0 },
            "seven_day": { "utilization": 6.0 },
            "limits": { "kind": "weekly_scoped" }
        }),
    ] {
        let snapshot = snapshot_from(&body, 0);
        assert_eq!(snapshot.source, UsageSource::Account, "{body}");
        assert_eq!(snapshot.windows.len(), 2, "{body}");
    }
}

/// `percent` is a whole number where `utilization` is fractional, and it comes
/// off the same undocumented body. It clamps like everything else. R-3.
#[test]
fn the_scoped_number_is_clamped_and_its_reset_may_be_missing() {
    let body = json!({
        "five_hour": { "utilization": 5.0 },
        "seven_day": { "utilization": 6.0 },
        "limits": [{ "kind": "weekly_scoped", "group": "weekly", "percent": 140,
                     "scope": { "model": { "display_name": "Fable" } } }]
    });
    let snapshot = snapshot_from(&body, 0);

    assert_eq!(snapshot.windows[2].used_pct, 100.0);
    assert_eq!(snapshot.windows[2].resets_at, None);
    assert_eq!(snapshot.windows[2].scope.as_deref(), Some("Fable"));
}
