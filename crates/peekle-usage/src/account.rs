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

use crate::credentials::{access_token, CredentialError, CredentialStore};
use crate::UsageProvider;

const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
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
            windows: vec![five, week],
            source: UsageSource::Account,
            reason: None,
            fetched_at,
            // Stamped centrally where snapshots are handed to the island, not
            // known here. tech.md 6.4.
            keychain_granted: false,
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

pub fn unavailable(reason: UsageUnavailable, fetched_at: i64) -> UsageSnapshot {
    UsageSnapshot {
        windows: Vec::new(),
        source: UsageSource::Unavailable,
        reason: Some(reason),
        fetched_at,
        keychain_granted: false,
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

/// Why one request did not come back with a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AskError {
    /// The server answered, with this code.
    Status(u16),
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
    fn ask(&self, token: &str, budget: Duration) -> Result<UsageSnapshot, AskError> {
        let response = ureq::get(ENDPOINT)
            .config()
            .timeout_global(Some(budget))
            .build()
            .header("user-agent", &self.agent)
            // The only place the token is used, and it goes nowhere else.
            .header("authorization", &format!("Bearer {token}"))
            .call();

        match response {
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
            Err(ureq::Error::StatusCode(code)) => Err(AskError::Status(code)),
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
            // An expired token is not worth an agent turn to refresh: Claude
            // Code rewrites the Keychain entry the next time it runs, and the
            // next poll reads it. tech.md 6.4 step 4.
            Err(AskError::Status(401 | 403)) => {
                unavailable(UsageUnavailable::NotLoggedIn, now_ms())
            }
            // Reached and answered: asked to wait, not unreachable. Calling it
            // a network failure would offer a Reconnect that makes it worse.
            Err(AskError::Status(429)) => unavailable(UsageUnavailable::RateLimited, now_ms()),
            Err(AskError::Offline) => unavailable(UsageUnavailable::Offline, now_ms()),
            Err(_) => unavailable(UsageUnavailable::Network, now_ms()),
        }
    }
}
