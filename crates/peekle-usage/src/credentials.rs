//! Keychain access. tech.md 6.4 and 7.
//!
//! The real store shells out to `/usr/bin/security` rather than calling
//! SecItem in process. Keychain ACLs bind to the requesting binary, and
//! `security` is a stably signed Apple tool, so an Always Allow granted once
//! survives app updates. For an ad-hoc signed binary that is the difference
//! between one dialog and a dialog on every launch.

use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;

/// Exit codes of `security find-generic-password`. tech.md 6.4.
#[cfg(target_os = "macos")]
const EXIT_DENIED: i32 = 128;
#[cfg(target_os = "macos")]
const EXIT_NO_ENTRY: i32 = 44;

#[cfg(target_os = "macos")]
const SERVICE: &str = "Claude Code-credentials";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    /// The user pressed Deny.
    Denied,
    /// No entry, so the user never logged in through the CLI.
    NotLoggedIn,
    /// Anything else, treated as transient.
    Transient,
}

pub trait CredentialStore: Send + Sync + 'static {
    /// Returns the raw credentials JSON. Never logged, never returned in an
    /// error message. tech.md rule 11.
    fn read(&self) -> Result<String, CredentialError>;
}

/// The real one on macOS. Only ever called from a user action.
#[cfg(target_os = "macos")]
pub struct SecurityToolStore {
    account: String,
}

#[cfg(target_os = "macos")]
impl SecurityToolStore {
    pub fn new(account: impl Into<String>) -> Self {
        Self {
            account: account.into(),
        }
    }

    pub fn for_current_user() -> Self {
        Self::new(std::env::var("USER").unwrap_or_default())
    }
}

#[cfg(target_os = "macos")]
impl CredentialStore for SecurityToolStore {
    fn read(&self) -> Result<String, CredentialError> {
        let output = Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                SERVICE,
                "-a",
                &self.account,
                "-w",
            ])
            .output()
            .map_err(|_| CredentialError::Transient)?;

        match output.status.code() {
            Some(0) => {
                let raw = String::from_utf8_lossy(&output.stdout);
                Ok(raw.trim_end_matches('\n').to_string())
            }
            Some(EXIT_DENIED) => Err(CredentialError::Denied),
            Some(EXIT_NO_ENTRY) => Err(CredentialError::NotLoggedIn),
            _ => Err(CredentialError::Transient),
        }
    }
}

/// The real one on Windows and Linux: the file Claude Code itself keeps its
/// OAuth in, `~/.claude/.credentials.json`. The same JSON the Keychain blob
/// carries, so the same `access_token` reads it. No dialog stands in front of
/// a file, which is why `keychain_granted` (6.4) is set by the first read
/// that succeeds. tech.md 6.27.
pub struct ClaudeFileStore {
    path: PathBuf,
}

impl ClaudeFileStore {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_current_user() -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::at(home.join(".claude").join(".credentials.json"))
    }
}

impl CredentialStore for ClaudeFileStore {
    fn read(&self) -> Result<String, CredentialError> {
        match std::fs::read_to_string(&self.path) {
            Ok(raw) => Ok(raw),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(CredentialError::NotLoggedIn)
            }
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                Err(CredentialError::Denied)
            }
            Err(_) => Err(CredentialError::Transient),
        }
    }
}

/// The store the app uses on this platform. tech.md 6.27.
#[cfg(target_os = "macos")]
pub type SystemCredentialStore = SecurityToolStore;
#[cfg(not(target_os = "macos"))]
pub type SystemCredentialStore = ClaudeFileStore;

/// Fake store backed by a temp file. Never raises a Keychain dialog, so tests
/// and dev sessions never wait on one. tech.md section 7.
pub struct FakeCredentialStore {
    path: PathBuf,
    outcome: Option<CredentialError>,
}

impl FakeCredentialStore {
    pub fn with_contents(dir: &std::path::Path, contents: &str) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("credentials.json");
        std::fs::write(&path, contents)?;
        Ok(Self {
            path,
            outcome: None,
        })
    }

    /// Reproduces any documented failure, so each error path has a test.
    pub fn failing(outcome: CredentialError) -> Self {
        Self {
            path: PathBuf::new(),
            outcome: Some(outcome),
        }
    }
}

impl CredentialStore for FakeCredentialStore {
    fn read(&self) -> Result<String, CredentialError> {
        if let Some(outcome) = self.outcome {
            return Err(outcome);
        }
        std::fs::read_to_string(&self.path).map_err(|_| CredentialError::NotLoggedIn)
    }
}

/// Pulls the OAuth token out of the credentials JSON. Returns nothing rather
/// than a partial value when the shape is unfamiliar.
pub fn access_token(raw: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(raw).ok()?;
    parsed
        .get("claudeAiOauth")?
        .get("accessToken")?
        .as_str()
        .map(str::to_owned)
}

/// When that token stops being accepted, unix ms, as Claude Code writes it.
///
/// The same field the CLI reads before every authenticated call, and the
/// reason it almost never sees a 401: it refreshes on the clock rather than
/// on a refusal. Absent in an entry written by an older CLI, which is not an
/// error -- it only means the clock cannot be consulted. tech.md 6.4.
pub fn expires_at(raw: &str) -> Option<i64> {
    let parsed: serde_json::Value = serde_json::from_str(raw).ok()?;
    parsed.get("claudeAiOauth")?.get("expiresAt")?.as_i64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("peekle-cred-{}", Ulid::generate()))
    }

    #[test]
    fn reads_the_token_out_of_the_credentials_blob() {
        let dir = temp_dir();
        let store = FakeCredentialStore::with_contents(
            &dir,
            r#"{"claudeAiOauth":{"accessToken":"sk-secret","expiresAt":1}}"#,
        )
        .unwrap();

        let token = access_token(&store.read().unwrap()).unwrap();
        assert_eq!(token, "sk-secret");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unfamiliar_shape_yields_no_token_rather_than_a_guess() {
        assert_eq!(access_token("{}"), None);
        assert_eq!(access_token(r#"{"claudeAiOauth":{}}"#), None);
        assert_eq!(access_token("not json"), None);
    }

    /// Each documented exit code has to reach the UI as its own reason.
    /// The file Claude Code keeps on Windows and Linux reads like the Keychain
    /// blob, and a missing file is "never logged in", not a fault.
    #[test]
    fn the_file_store_reads_the_blob_and_names_a_missing_one() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).expect("made the dir");
        let path = dir.join(".credentials.json");
        std::fs::write(
            &path,
            r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-x","expiresAt":1}}"#,
        )
        .expect("wrote");
        let store = ClaudeFileStore::at(path.clone());
        assert_eq!(
            access_token(&store.read().expect("read")),
            Some("sk-ant-oat01-x".to_string())
        );

        std::fs::remove_file(&path).expect("removed");
        assert!(matches!(store.read(), Err(CredentialError::NotLoggedIn)));
    }

    #[test]
    fn every_failure_mode_is_reachable() {
        for outcome in [
            CredentialError::Denied,
            CredentialError::NotLoggedIn,
            CredentialError::Transient,
        ] {
            assert_eq!(FakeCredentialStore::failing(outcome).read(), Err(outcome));
        }
    }
}
