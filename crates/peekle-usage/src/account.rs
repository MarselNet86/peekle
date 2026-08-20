//! Usage from the user's own account. tech.md 6.4.
//!
//! One free GET to the same endpoint Claude Code draws its own `/usage` screen
//! from, so the numbers here agree with the numbers the user sees there. The
//! previous path scraped rate limit headers off a real `/v1/messages` request,
//! which charged the very window it was measuring.
//!
//! Nothing here ever logs the token. Rule 11 allows its length and nothing
//! else, and an error message counts as a log.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use peekle_core::types::{
    UsageSnapshot, UsageSource, UsageUnavailable, UsageWindow, UsageWindowStat,
};
use serde_json::Value;

use crate::credentials::{access_token, CredentialError, CredentialStore};
use crate::UsageProvider;

const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
const TIMEOUT: Duration = Duration::from_secs(10);

/// Turns the response body into a snapshot. Pure, so the whole contract of
/// step 3 is testable without a network.
///
/// `utilization` here is a percentage, `92.0` meaning ninety two percent.
/// Verified against a live response in core v18, where the same account showed
/// 92% on the `/usage` screen. The header path of R-3 reports a fraction
/// instead, and mixing the two scales would draw 0.92% for a full window.
///
/// A window that is missing, or that carries no number, yields dashes rather
/// than a number assembled out of whatever else was in the body.
pub fn snapshot_from(body: &Value, fetched_at: i64) -> UsageSnapshot {
    let five = window_stat(body, UsageWindow::FiveHour, "five_hour");
    let week = window_stat(body, UsageWindow::SevenDay, "seven_day");

    match (five, week) {
        (Some(five), Some(week)) => UsageSnapshot {
            windows: vec![five, week],
            source: UsageSource::Account,
            reason: None,
            fetched_at,
        },
        _ => unavailable(UsageUnavailable::Unsupported, fetched_at),
    }
}

fn window_stat(body: &Value, window: UsageWindow, key: &str) -> Option<UsageWindowStat> {
    let entry = body.get(key)?;
    let percent = entry.get("utilization")?.as_f64()? as f32;

    // A window with no reset is still a window: the bar draws, the countdown
    // stays empty. tech.md 6.4.
    let resets_at = entry
        .get("resets_at")
        .and_then(Value::as_str)
        .and_then(iso_seconds);

    // UsageWindowStat clamps, so an out of range percentage cannot reach the UI.
    Some(UsageWindowStat::new(window, percent, resets_at))
}

/// Seconds since the epoch for an ISO 8601 timestamp such as
/// `2026-08-20T16:30:00.366734+00:00`.
///
/// Hand rolled rather than pulled from a date crate: this is the only date the
/// product parses, the shape is fixed by the server, and a wrong answer here is
/// a wrong countdown rather than a crash. Fractional seconds are dropped and
/// the offset is applied.
pub fn iso_seconds(raw: &str) -> Option<i64> {
    let bytes = raw.as_bytes();
    if bytes.len() < 19 {
        return None;
    }

    let num = |from: usize, to: usize| raw.get(from..to)?.parse::<i64>().ok();
    let year = num(0, 4)?;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    let hour = num(11, 13)?;
    let minute = num(14, 16)?;
    let second = num(17, 19)?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    Some(seconds - offset_seconds(&raw[19..])?)
}

/// The trailing offset, in seconds east of UTC. `Z`, nothing at all, and
/// `+00:00` all mean the same thing.
fn offset_seconds(rest: &str) -> Option<i64> {
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let sign = match rest.as_bytes().first() {
        None | Some(b'Z' | b'z') => return Some(0),
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return None,
    };

    let hours = rest.get(1..3)?.parse::<i64>().ok()?;
    // `+0130` and `+01:30` are both legal, and both appear in the wild.
    let minutes = rest
        .get(3..)
        .map(|tail| tail.trim_start_matches(':'))
        .filter(|tail| !tail.is_empty())
        .map_or(Some(0), |tail| tail.get(0..2)?.parse::<i64>().ok())?;

    Some(sign * (hours * 3600 + minutes * 60))
}

/// Days from the epoch for a civil date. Howard Hinnant's algorithm, valid for
/// any proleptic Gregorian date, which is more than a reset timestamp needs.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub fn unavailable(reason: UsageUnavailable, fetched_at: i64) -> UsageSnapshot {
    UsageSnapshot {
        windows: Vec::new(),
        source: UsageSource::Unavailable,
        reason: Some(reason),
        fetched_at,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// The real provider.
pub struct AccountUsage {
    store: Box<dyn CredentialStore>,
    agent: String,
}

impl AccountUsage {
    pub fn new(store: Box<dyn CredentialStore>, app_version: &str) -> Self {
        Self {
            store,
            agent: format!("peekle/{app_version}"),
        }
    }

    fn token(&self) -> Result<String, UsageUnavailable> {
        match self.store.read() {
            Ok(raw) => access_token(&raw).ok_or(UsageUnavailable::NotLoggedIn),
            Err(CredentialError::Denied) => Err(UsageUnavailable::Denied),
            Err(CredentialError::NotLoggedIn) => Err(UsageUnavailable::NotLoggedIn),
            Err(CredentialError::Transient) => Err(UsageUnavailable::Network),
        }
    }

    /// One request. Returns the snapshot, or the status code when the server
    /// refused, so the caller can tell an expired token from a dead network.
    fn ask(&self, token: &str) -> Result<UsageSnapshot, Option<u16>> {
        let response = ureq::get(ENDPOINT)
            .config()
            .timeout_global(Some(TIMEOUT))
            .build()
            .header("user-agent", &self.agent)
            // The only place the token is used, and it goes nowhere else.
            .header("authorization", &format!("Bearer {token}"))
            .call();

        match response {
            Ok(mut response) => {
                let raw = response.body_mut().read_to_string().map_err(|err| {
                    tracing::warn!(error = %err, "could not read the usage body");
                    None
                })?;
                let body: Value = serde_json::from_str(&raw).map_err(|err| {
                    tracing::warn!(error = %err, bytes = raw.len(), "usage body is not json");
                    None
                })?;
                Ok(snapshot_from(&body, now_ms()))
            }
            Err(ureq::Error::StatusCode(code)) => Err(Some(code)),
            Err(err) => {
                // The error carries the endpoint and the transport failure,
                // never a header, so the token cannot ride along. Swallowing
                // it left `could not reach the API` with nothing behind it.
                tracing::warn!(error = %err, endpoint = ENDPOINT, "usage request failed");
                Err(None)
            }
        }
    }
}

impl UsageProvider for AccountUsage {
    fn snapshot(&self) -> UsageSnapshot {
        let token = match self.token() {
            Ok(token) => token,
            Err(reason) => return unavailable(reason, now_ms()),
        };
        tracing::debug!(token_len = token.len(), "reading usage for the account");

        match self.ask(&token) {
            Ok(snapshot) => snapshot,
            // An expired token is not worth an agent turn to refresh: Claude
            // Code rewrites the Keychain entry the next time it runs, and the
            // next poll reads it. tech.md 6.4 step 4.
            Err(Some(401 | 403)) => unavailable(UsageUnavailable::NotLoggedIn, now_ms()),
            Err(_) => unavailable(UsageUnavailable::Network, now_ms()),
        }
    }
}
