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

use peekle_core::time::iso_seconds;
use peekle_core::types::{
    UsageSnapshot, UsageSource, UsageUnavailable, UsageWindow, UsageWindowStat,
};
use serde_json::Value;

use crate::credentials::{access_token, expires_at, CredentialError, CredentialStore};
use crate::UsageProvider;

const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";

/// The beta every claude.ai OAuth request of the CLI carries.
///
/// Read out of `claude` 2.1.263: the branch that builds headers for
/// `auth === "claude-ai-oauth"` sends exactly two, `Authorization: Bearer ...`
/// and `anthropic-beta: oauth-2025-04-20`, and `/api/oauth/usage` is one of
/// those requests. A bearer token minted for this scope is not accepted
/// without it, which is what a `401` here has been all along. tech.md 6.4.
const OAUTH_BETA: &str = "oauth-2025-04-20";

/// How close to expiry the CLI stops trusting a token: it refreshes below 120
/// seconds and blocks on a refresh below 30. Peekle cannot refresh -- the
/// entry is Claude Code's and minting against its client id would be passing
/// Peekle off as Claude Code -- but it can decline to spend a request on a
/// credential the CLI itself would have replaced first. tech.md 6.4.
const SKEW_MS: i64 = 30_000;
/// What the background poll allows one request. A press allows less: see
/// `snapshot_within`. tech.md 6.4.
pub const TIMEOUT: Duration = Duration::from_secs(10);

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
            // The scoped week is optional and comes last: a plan that counts
            // no model apart never reports one, and the two windows above are
            // what every account has. tech.md 6.4.
            windows: [Some(five), Some(week), scoped_stat(body)]
                .into_iter()
                .flatten()
                .collect(),
            source: UsageSource::Account,
            reason: None,
            fetched_at,
            // Stamped centrally where snapshots are handed to the island, not
            // known here. tech.md 6.4.
            keychain_granted: false,
            retry_after_ms: None,
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

/// The week a plan counts for one model on its own.
///
/// It has no key of its own at the top level -- it arrives as a row of
/// `limits`, which is the one thing that array is read for. Two marks make a
/// row that one: it is weekly, and it names the model it is scoped to. The
/// name is the server's (`Fable`), never a guess from here: which models a
/// plan counts apart is not ours to decide. `percent` here is a whole number
/// on the same 0..100 scale as `utilization`. tech.md 6.4.
///
/// The other kinds are left alone. `session` and `weekly_all` are the numbers
/// that already arrived as `five_hour` and `seven_day`, and one number read
/// from two places drifts apart sooner or later.
fn scoped_stat(body: &Value) -> Option<UsageWindowStat> {
    body.get("limits")?.as_array()?.iter().find_map(|limit| {
        if limit.get("group")?.as_str()? != "weekly" {
            return None;
        }
        let scope = limit
            .get("scope")?
            .get("model")?
            .get("display_name")?
            .as_str()?;
        let percent = limit.get("percent")?.as_f64()? as f32;
        let resets_at = limit
            .get("resets_at")
            .and_then(Value::as_str)
            .and_then(iso_seconds);

        Some(UsageWindowStat::scoped(percent, resets_at, scope))
    })
}

pub fn unavailable(reason: UsageUnavailable, fetched_at: i64) -> UsageSnapshot {
    UsageSnapshot {
        windows: Vec::new(),
        source: UsageSource::Unavailable,
        reason: Some(reason),
        fetched_at,
        keychain_granted: false,
        retry_after_ms: None,
    }
}

/// The same, carrying how long the endpoint asked to be left alone.
pub fn rate_limited(fetched_at: i64, retry_after_ms: Option<i64>) -> UsageSnapshot {
    UsageSnapshot {
        retry_after_ms,
        ..unavailable(UsageUnavailable::RateLimited, fetched_at)
    }
}

/// `retry-after` as a duration. Seconds is the form this endpoint sends, seen
/// live as `retry-after: 1456`; the HTTP-date form is not read, because a
/// clock this side disagreeing with the server's would be a wait invented
/// here. A header that is not a plain count of seconds is no header at all,
/// and the poll falls back to its own interval. tech.md 6.4.
pub fn retry_after_ms(header: Option<&str>) -> Option<i64> {
    let secs: i64 = header?.trim().parse().ok()?;
    (secs > 0).then(|| secs.saturating_mul(1000))
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

/// Why one request did not come back with a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AskError {
    /// The server answered, with this code, and with what it said about
    /// coming back: `retry-after` in milliseconds, when it sent one.
    Status(u16, Option<i64>),
    /// Nothing to connect to and no name to resolve: the machine is off the
    /// network. tech.md 6.4.
    Offline,
    /// Something is out there, but it did not answer in the time allowed.
    Unreachable,
}

impl AccountUsage {
    pub fn new(store: Box<dyn CredentialStore>, app_version: &str) -> Self {
        Self {
            store,
            agent: format!("peekle/{app_version}"),
        }
    }

    /// The token Claude Code last wrote, and only while it is still good for
    /// something.
    ///
    /// The CLI checks the clock before every call and refreshes rather than
    /// waiting to be refused. Peekle copies the check and stops there: a
    /// token inside the skew is spent, the request would come back `401`, and
    /// the honest answer is the one the CLI gives itself in the same place --
    /// there is no usable claude.ai login until it runs again. tech.md 6.4.
    fn token(&self) -> Result<String, UsageUnavailable> {
        let raw = match self.store.read() {
            Ok(raw) => raw,
            Err(CredentialError::Denied) => return Err(UsageUnavailable::Denied),
            Err(CredentialError::NotLoggedIn) => return Err(UsageUnavailable::NotLoggedIn),
            Err(CredentialError::Transient) => return Err(UsageUnavailable::Network),
        };

        if let Some(expiry) = expires_at(&raw) {
            if now_ms() + SKEW_MS >= expiry {
                tracing::debug!(
                    late_by_ms = now_ms() - expiry,
                    "the credential is spent; Claude Code refreshes it on its next run"
                );
                return Err(UsageUnavailable::NotLoggedIn);
            }
        }

        access_token(&raw).ok_or(UsageUnavailable::NotLoggedIn)
    }

    /// One request. Returns the snapshot, or the status code when the server
    /// refused, so the caller can tell an expired token from a dead network.
    fn ask(&self, token: &str, budget: Duration) -> Result<UsageSnapshot, AskError> {
        let response = ureq::get(ENDPOINT)
            .config()
            .timeout_global(Some(budget))
            // A refusal is an answer and it carries what the server wants
            // known: `429` says how long to stay away, and turning it into a
            // bare error throws that away. tech.md 6.4.
            .http_status_as_error(false)
            .build()
            .header("user-agent", &self.agent)
            // The only place the token is used, and it goes nowhere else.
            .header("authorization", &format!("Bearer {token}"))
            // The pair the CLI sends, copied rather than invented. tech.md 6.4.
            .header("anthropic-beta", OAUTH_BETA)
            .call();

        match response {
            Ok(response) if response.status() != 200 => {
                let after = retry_after_ms(
                    response
                        .headers()
                        .get("retry-after")
                        .and_then(|value| value.to_str().ok()),
                );
                Err(AskError::Status(response.status().as_u16(), after))
            }
            Ok(mut response) => {
                let raw = response.body_mut().read_to_string().map_err(|err| {
                    tracing::warn!(error = %err, "could not read the usage body");
                    AskError::Unreachable
                })?;
                let body: Value = serde_json::from_str(&raw).map_err(|err| {
                    tracing::warn!(error = %err, bytes = raw.len(), "usage body is not json");
                    AskError::Unreachable
                })?;
                Ok(snapshot_from(&body, now_ms()))
            }
            // Kept for a build that leaves `http_status_as_error` on: the
            // code still travels, the header does not.
            Err(ureq::Error::StatusCode(code)) => Err(AskError::Status(code, None)),
            // Nothing answered in time. The machine may well be online: a
            // captive portal and a silent endpoint look the same from here,
            // and both cost the whole budget. tech.md 6.4.
            Err(err @ ureq::Error::Timeout(_)) => {
                tracing::warn!(error = %err, endpoint = ENDPOINT, "usage request timed out");
                Err(AskError::Unreachable)
            }
            // The name did not resolve, or there was nowhere to connect to.
            // Measured at a tenth of a second, and it is the one failure the
            // person in front of the screen can fix. tech.md 6.4.
            Err(err) => {
                // The error carries the endpoint and the transport failure,
                // never a header, so the token cannot ride along.
                tracing::warn!(error = %err, endpoint = ENDPOINT, "usage request failed");
                Err(AskError::Offline)
            }
        }
    }
}

impl UsageProvider for AccountUsage {
    fn snapshot(&self) -> UsageSnapshot {
        self.snapshot_within(TIMEOUT)
    }

    /// The same read, bounded by what the caller can wait for.
    ///
    /// A press has to answer while a finger is still on the button, and a hole
    /// in the network costs the whole timeout: measured, a blackholed address
    /// takes every second of it, while an unresolvable name fails in a tenth.
    /// tech.md 6.4.
    fn snapshot_within(&self, budget: Duration) -> UsageSnapshot {
        let token = match self.token() {
            Ok(token) => token,
            Err(reason) => return unavailable(reason, now_ms()),
        };
        tracing::debug!(token_len = token.len(), "reading usage for the account");

        match self.ask(&token, budget) {
            Ok(snapshot) => snapshot,
            // What the CLI does with a `401` on this very endpoint: refresh
            // the token and send the request again, once. Refreshing is
            // Claude Code's to do, and the Keychain entry is where it puts
            // the result, so Peekle's refresh is a second read of it. A token
            // that came back unchanged has already been refused, and asking
            // twice with it would only spend the budget. tech.md 6.4.
            Err(AskError::Status(401 | 403, _)) => match self.token() {
                Ok(fresh) if fresh != token => match self.ask(&fresh, budget) {
                    Ok(snapshot) => snapshot,
                    _ => unavailable(UsageUnavailable::NotLoggedIn, now_ms()),
                },
                _ => unavailable(UsageUnavailable::NotLoggedIn, now_ms()),
            },
            // Reached and answered: asked to wait, not unreachable. Calling it
            // a network failure would offer a Reconnect that makes it worse.
            // Reached and answered: asked to wait, not unreachable, and the
            // server said for how long. tech.md 6.4.
            Err(AskError::Status(429, after)) => rate_limited(now_ms(), after),
            Err(AskError::Offline) => unavailable(UsageUnavailable::Offline, now_ms()),
            Err(_) => unavailable(UsageUnavailable::Network, now_ms()),
        }
    }
}

#[cfg(test)]
mod retry_tests {
    use super::{rate_limited, retry_after_ms};
    use peekle_core::types::UsageUnavailable;

    /// The header as the endpoint sent it on 2026-09-09, when it refused a
    /// plain request from this machine: `retry-after: 1456`. tech.md 6.4.
    #[test]
    fn the_captured_header_becomes_a_wait() {
        assert_eq!(retry_after_ms(Some("1456")), Some(1_456_000));
        assert_eq!(retry_after_ms(Some(" 30 ")), Some(30_000));
    }

    /// No header, a date, a zero or nonsense: the poll keeps its own interval
    /// rather than inventing one. A date would need this clock to agree with
    /// the server's, and it does not have to.
    #[test]
    fn anything_that_is_not_a_count_of_seconds_is_no_wait_at_all() {
        for header in [
            None,
            Some("0"),
            Some("-5"),
            Some("Wed, 09 Sep 2026 15:00:00 GMT"),
        ] {
            assert_eq!(retry_after_ms(header), None, "{header:?}");
        }
    }

    #[test]
    fn a_rate_limited_snapshot_carries_the_wait_and_nothing_else() {
        let snapshot = rate_limited(1_000, Some(1_456_000));

        assert_eq!(snapshot.reason, Some(UsageUnavailable::RateLimited));
        assert_eq!(snapshot.retry_after_ms, Some(1_456_000));
        assert!(snapshot.windows.is_empty());
    }
}
