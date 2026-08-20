//! Usage from the user's own account. tech.md 6.4.
//!
//! There is no quota API. The numbers ride on the response headers of any
//! request made with the user's OAuth token, so this makes the smallest
//! request there is and throws the body away.
//!
//! Nothing here ever logs the token. Rule 11 allows its length and nothing
//! else, and an error message counts as a log.

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use peekle_core::types::{
    UsageSnapshot, UsageSource, UsageUnavailable, UsageWindow, UsageWindowStat,
};

use crate::credentials::{access_token, CredentialError, CredentialStore};
use crate::UsageProvider;

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const TIMEOUT: Duration = Duration::from_secs(10);
/// One refresh attempt per five minutes, however often the token is rejected.
const REFRESH_COOLDOWN: Duration = Duration::from_secs(300);

/// Where `claude` might be, in the order tech.md 6.4 gives.
const CLI_CANDIDATES: &[&str] = &[
    ".local/bin/claude",
    "/opt/homebrew/bin/claude",
    "/usr/local/bin/claude",
];

/// The four headers the bars are built from. tech.md 6.4.
const H_5H_UTIL: &str = "anthropic-ratelimit-unified-5h-utilization";
const H_5H_RESET: &str = "anthropic-ratelimit-unified-5h-reset";
const H_7D_UTIL: &str = "anthropic-ratelimit-unified-7d-utilization";
const H_7D_RESET: &str = "anthropic-ratelimit-unified-7d-reset";

/// Reads a header by name, whatever case the server used.
pub trait Headers {
    fn get(&self, name: &str) -> Option<String>;
}

/// Turns the headers into a snapshot. Pure, so the whole contract of step 3 is
/// testable without a network.
///
/// Utilization arrives as a fraction: `0.17` means seventeen percent. Verified
/// on a live response in core v13. A missing header of the four means the
/// shape changed, and then the honest answer is dashes rather than a number
/// built out of whatever else was there. tech.md 6.4 and R-3.
pub fn snapshot_from(headers: &dyn Headers, fetched_at: i64) -> UsageSnapshot {
    let five = window_stat(headers, UsageWindow::FiveHour, H_5H_UTIL, H_5H_RESET);
    let week = window_stat(headers, UsageWindow::SevenDay, H_7D_UTIL, H_7D_RESET);

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

fn window_stat(
    headers: &dyn Headers,
    window: UsageWindow,
    util: &str,
    reset: &str,
) -> Option<UsageWindowStat> {
    let fraction: f32 = headers.get(util)?.trim().parse().ok()?;
    let resets_at: i64 = headers.get(reset)?.trim().parse().ok()?;

    // UsageWindowStat clamps, so an out of range fraction cannot reach the UI.
    Some(UsageWindowStat::new(
        window,
        fraction * 100.0,
        Some(resets_at),
    ))
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

struct UreqHeaders<'a, T>(&'a http::Response<T>);

impl<T> Headers for UreqHeaders<'_, T> {
    fn get(&self, name: &str) -> Option<String> {
        self.0
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    }
}

/// The real provider.
pub struct AccountUsage {
    store: Box<dyn CredentialStore>,
    agent: String,
    last_refresh: Mutex<Option<Instant>>,
}

impl AccountUsage {
    pub fn new(store: Box<dyn CredentialStore>, app_version: &str) -> Self {
        Self {
            store,
            agent: format!("claude-code/{app_version}"),
            last_refresh: Mutex::new(None),
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
    /// refused, so the caller can decide whether a refresh is worth trying.
    fn ask(&self, token: &str) -> Result<UsageSnapshot, Option<u16>> {
        let body = r#"{"model":"claude-haiku-4-5-20251001","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}"#;

        let response = ureq::post(ENDPOINT)
            .config()
            .timeout_global(Some(TIMEOUT))
            .build()
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("content-type", "application/json")
            .header("user-agent", &self.agent)
            // The only place the token is used, and it goes nowhere else.
            .header("authorization", &format!("Bearer {token}"))
            .send(body);

        match response {
            Ok(response) => Ok(snapshot_from(&UreqHeaders(&response), now_ms())),
            Err(ureq::Error::StatusCode(code)) => Err(Some(code)),
            Err(_) => Err(None),
        }
    }

    /// Lets the CLI refresh the Keychain entry, at most once per cooldown.
    /// Reports whether it ran, not whether it worked: the retry reads the
    /// entry again either way.
    fn try_refresh(&self) -> bool {
        let mut last = self.last_refresh.lock().unwrap_or_else(|e| e.into_inner());

        if last.is_some_and(|at| at.elapsed() < REFRESH_COOLDOWN) {
            return false;
        }
        let Some(cli) = claude_path() else {
            return false;
        };
        *last = Some(Instant::now());

        let _ = std::process::Command::new(cli)
            .args(["-p", "hi", "--max-budget-usd", "0.01"])
            .output();
        true
    }
}

/// First candidate that exists. tech.md 6.4.
fn claude_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    CLI_CANDIDATES
        .iter()
        .map(|candidate| {
            if candidate.starts_with('/') {
                std::path::PathBuf::from(candidate)
            } else {
                std::path::Path::new(&home).join(candidate)
            }
        })
        .find(|path| path.exists())
}

impl UsageProvider for AccountUsage {
    fn snapshot(&self) -> UsageSnapshot {
        let token = match self.token() {
            Ok(token) => token,
            Err(reason) => return unavailable(reason, now_ms()),
        };

        match self.ask(&token) {
            Ok(snapshot) => snapshot,
            // Expired. Let the CLI refresh the entry once, then read and retry
            // exactly once. tech.md 6.4 step 4.
            Err(Some(401)) | Err(Some(403)) => {
                if !self.try_refresh() {
                    return unavailable(UsageUnavailable::NotLoggedIn, now_ms());
                }
                match self.token() {
                    Ok(fresh) => self
                        .ask(&fresh)
                        .unwrap_or_else(|_| unavailable(UsageUnavailable::NotLoggedIn, now_ms())),
                    Err(reason) => unavailable(reason, now_ms()),
                }
            }
            Err(_) => unavailable(UsageUnavailable::Network, now_ms()),
        }
    }
}
