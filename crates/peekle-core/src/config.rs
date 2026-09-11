//! Config file handling. tech.md section 6.8 is the source of truth for every
//! key and default. Unknown keys are kept out of the way and warned about, not
//! rejected: a newer Peekle must not brick an older config and the reverse.

use std::collections::BTreeMap;
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// `~/Library/Application Support/peekle/config.toml`
pub const CONFIG_FILE: &str = "config.toml";
/// Port is duplicated here so `doctor` and `status` can find the server
/// without parsing the config.
pub const PORT_FILE: &str = ".peekle/port";

#[cfg(unix)]
const OWNER_ONLY: u32 = 0o600;
const MAX_FEED_VISIBLE_ROWS: u8 = 6;
const MIN_SHOT_POLL_MS: u64 = 100;
const MAX_SHOT_POLL_MS: u64 = 5_000;
const MIN_SHOT_OFFER_SECS: u32 = 1;
/// An offer holds the Up arrow away from every other application, so its
/// upper bound is a product decision, not a preference. tech.md R-14.
const MAX_SHOT_OFFER_SECS: u32 = 30;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config io failed: {0}")]
    Io(#[from] io::Error),
    #[error("config is not valid toml: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config could not be serialized: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("no home directory for this user")]
    NoHome,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub hotkey: HotkeyConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub usage: UsageConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub shots: ShotsConfig,
    #[serde(default)]
    pub notify: NotifyConfig,

    /// Sections this build does not know. Kept so a round trip does not delete
    /// a newer Peekle's settings.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unknown: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub port: u16,
    /// 32 hex characters, minted once. Never logged, never sent anywhere.
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub toggle: String,
    /// Empty means do not register.
    pub recall: String,
    /// Agreement to attach a screenshot. Held only while an offer stands, and
    /// dropped on every path that settles one, because a modifierless key
    /// taken from the whole system for good would be a fault. Empty switches
    /// the offer off. tech.md 6.9 and 6.13.
    pub attach: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub island_opacity: f32,
    /// Always false: the island is opaque black, blur would give it away as a
    /// window on top of the system rather than part of the bezel. tech.md 6.10.
    pub blur: bool,
    pub feed_visible_rows: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UsageConfig {
    pub enabled: bool,
    pub provider: UsageProviderKind,
    /// Set by the app when the user denies Keychain access. Cleared by hand.
    pub keychain_denied: bool,
    /// Set by the app after one successful read from `request_usage_access`.
    /// The background poll stays off until then, because reading the Keychain
    /// before a grant would raise the dialog the app is forbidden to raise on
    /// its own. tech.md 6.4 and rule 12.
    pub keychain_granted: bool,
    /// Whether a resting island shows the percent when the five hour window
    /// steps into a new ten. The ring stands there all day; the number is an
    /// event, and it appears only when there is one. tech.md 6.18.
    pub badge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageProviderKind {
    Account,
    Fake,
    Off,
}

/// The banner a finished turn puts on the screen. tech.md 6.17.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifyConfig {
    /// Off until the user turns it on and macOS grants the permission. Nothing
    /// else sets it: posting the first banner raises a system dialog, and rule
    /// 12 forbids raising one without a person asking for it. The same
    /// discipline `keychain_granted` lives by. tech.md 6.17.
    pub enabled: bool,
}

/// Watching the pasteboard for screenshots. tech.md 6.13.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShotsConfig {
    pub enabled: bool,
    /// How long an offer stands before it settles itself.
    pub offer_secs: u32,
    /// How often the pasteboard change count is read. The tick costs one call
    /// and touches no contents; the key exists so a slow machine can slacken
    /// it, not so it gets tuned. tech.md 6.8.
    pub poll_ms: u64,
    /// How many written screenshots the cache keeps.
    pub keep: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub enabled: bool,
    /// How long the permission hook waits for a decision. Held far below the
    /// hook timeout of 86400s so Peekle gives up first and hands the question
    /// back to the terminal. tech.md 6.8.
    pub permission_wait_secs: u32,
    /// How long a sent reply waits for `UserPromptSubmit` before it is called
    /// undelivered. tech.md 6.3.
    pub delivery_confirm_secs: u32,
    /// Window size of the pty an owned session runs in. Invisible in the feed:
    /// Claude Code wraps its TUI to it, and the TUI is drained and discarded.
    /// The keys exist so a zero size can be ruled out. tech.md 6.5 and 6.8.
    pub pty_cols: u16,
    pub pty_rows: u16,
    /// What the context ring is measured against. Zero means the catalog
    /// decides, which is what it does for everyone who has not pinned their
    /// own window. tech.md 6.15.
    pub context_window: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 47821,
            token: generate_token(),
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            toggle: "Alt+Shift+KeyQ".to_string(),
            recall: String::new(),
            attach: "ArrowUp".to_string(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            island_opacity: 1.0,
            blur: false,
            feed_visible_rows: MAX_FEED_VISIBLE_ROWS,
        }
    }
}

impl Default for UsageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: UsageProviderKind::Account,
            keychain_denied: false,
            keychain_granted: false,
            badge: true,
        }
    }
}

impl Default for ShotsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            offer_secs: 5,
            poll_ms: 400,
            keep: 20,
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            permission_wait_secs: 300,
            delivery_confirm_secs: 20,
            pty_cols: 120,
            pty_rows: 40,
            context_window: 0,
        }
    }
}

impl Config {
    /// Parses a config, applying defaults for anything missing. Unknown keys
    /// survive the round trip; the caller decides how loudly to warn.
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let mut config: Config = toml::from_str(text)?;
        config.normalize();
        Ok(config)
    }

    /// Reads the config, or returns defaults with a fresh token when the file
    /// is missing.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_toml(&text),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(ConfigError::Io(err)),
        }
    }

    /// Writes the config with owner-only permissions. The token lives here, so
    /// the mode is part of the contract, not hygiene. On Windows there is no
    /// mode to set: the file lives under the user's profile, whose ACL already
    /// admits that user alone, and that is the same promise. tech.md 6.27.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text)?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(OWNER_ONLY))?;
        Ok(())
    }

    /// Names of sections this build does not understand.
    pub fn unknown_keys(&self) -> Vec<&str> {
        self.unknown.keys().map(String::as_str).collect()
    }

    fn normalize(&mut self) {
        if self.ui.feed_visible_rows > MAX_FEED_VISIBLE_ROWS {
            self.ui.feed_visible_rows = MAX_FEED_VISIBLE_ROWS;
        }
        if self.server.token.is_empty() {
            self.server.token = generate_token();
        }
        // A zero poll would spin a core reading a counter, and an offer with no
        // life at all could never be answered. Both are configuration mistakes
        // rather than choices, so they are clamped out of the way.
        self.shots.poll_ms = self.shots.poll_ms.clamp(MIN_SHOT_POLL_MS, MAX_SHOT_POLL_MS);
        self.shots.offer_secs = self
            .shots
            .offer_secs
            .clamp(MIN_SHOT_OFFER_SECS, MAX_SHOT_OFFER_SECS);
    }
}

/// 32 hex characters of randomness for the loopback token.
///
/// Built from the random halves of two ULIDs rather than pulling in an RNG
/// crate outside the frozen stack. A ULID's low 80 bits are random, so two of
/// them give 128 bits with no timestamp in the result.
pub fn generate_token() -> String {
    let a = Ulid::generate().0 as u64;
    let b = Ulid::generate().0 as u64;
    format!("{a:016x}{b:016x}")
}

/// `~/Library/Application Support/peekle/config.toml`
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let dirs = directories::ProjectDirs::from("", "", "peekle").ok_or(ConfigError::NoHome)?;
    Ok(dirs.config_dir().join(CONFIG_FILE))
}

/// `~/.peekle/port`
pub fn port_file_path() -> Result<PathBuf, ConfigError> {
    let home = directories::BaseDirs::new().ok_or(ConfigError::NoHome)?;
    Ok(home.home_dir().join(PORT_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = std::env::temp_dir().join(format!("peekle-cfg-{}", Ulid::generate()));
        let config = Config::load(&dir.join("config.toml")).unwrap();
        assert_eq!(config.server.port, 47821);
        assert_eq!(config.server.token.len(), 32);
        assert_eq!(config.hotkey.toggle, "Alt+Shift+KeyQ");
        assert_eq!(config.behavior.permission_wait_secs, 300);
        assert_eq!(config.behavior.pty_cols, 120);
        assert_eq!(config.behavior.pty_rows, 40);
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let config = Config::from_toml("[server]\nport = 5000\ntoken = \"abc\"\n").unwrap();
        assert_eq!(config.server.port, 5000);
        assert_eq!(config.ui.feed_visible_rows, 6);
        assert!(config.usage.enabled);
    }

    /// The badge is on out of the box, and a file that says nothing about it
    /// leaves it on: the island announces a ten, and there is nothing to grant
    /// and nothing to ask for. tech.md 6.18.
    #[test]
    fn the_usage_badge_is_on_until_a_file_says_otherwise() {
        assert!(Config::default().usage.badge);
        assert!(
            Config::from_toml("[usage]\nenabled = true\n")
                .unwrap()
                .usage
                .badge
        );

        let off = Config::from_toml("[usage]\nbadge = false\n").unwrap();
        assert!(!off.usage.badge);
        assert!(off.usage.enabled, "one key off is not the section off");
    }

    #[test]
    fn partial_section_keeps_the_other_keys_of_that_section() {
        let config = Config::from_toml("[server]\nport = 5000\n").unwrap();
        assert_eq!(config.server.port, 5000);
        assert_eq!(config.server.token.len(), 32);

        let config = Config::from_toml("[behavior]\nenabled = false\n").unwrap();
        assert!(!config.behavior.enabled);
        assert_eq!(config.behavior.permission_wait_secs, 300);
        assert_eq!(config.behavior.delivery_confirm_secs, 20);
    }

    /// Off on a fresh install, and nothing but the switch turns it on: the
    /// first banner is what makes macOS ask, and an app that asks on its own
    /// breaks rule 12. tech.md 6.17.
    #[test]
    fn turn_notices_are_off_until_the_user_says_otherwise() {
        assert!(!Config::default().notify.enabled);
        assert!(
            !Config::from_toml("[server]\nport = 5000\n")
                .unwrap()
                .notify
                .enabled
        );
    }

    /// What the switch wrote survives being read back, which is the whole job
    /// of the key: the answer to "is this on" outlives the run that answered
    /// it. tech.md 6.17.
    #[test]
    fn the_notify_switch_survives_a_round_trip() {
        let config = Config::from_toml("[notify]\nenabled = true\n").unwrap();
        assert!(config.notify.enabled);

        let written = toml::to_string_pretty(&config).unwrap();
        assert!(Config::from_toml(&written).unwrap().notify.enabled);
    }

    #[test]
    fn feed_visible_rows_clamps_to_six() {
        let config = Config::from_toml("[ui]\nfeed_visible_rows = 40\n").unwrap();
        assert_eq!(config.ui.feed_visible_rows, 6);
    }

    #[test]
    fn shots_defaults_are_the_ones_in_the_contract() {
        let config = Config::default();
        assert!(config.shots.enabled);
        assert_eq!(config.shots.offer_secs, 5);
        assert_eq!(config.shots.poll_ms, 400);
        assert_eq!(config.shots.keep, 20);
        assert_eq!(config.hotkey.attach, "ArrowUp");
    }

    /// An offer with no life could never be answered, and a zero poll would
    /// spin a core on a counter. tech.md 6.8.
    #[test]
    fn a_nonsense_shot_window_is_clamped_rather_than_obeyed() {
        let config = Config::from_toml("[shots]\npoll_ms = 0\noffer_secs = 0\n").unwrap();
        assert_eq!(config.shots.poll_ms, 100);
        assert_eq!(config.shots.offer_secs, 1);

        let config = Config::from_toml("[shots]\npoll_ms = 90000\noffer_secs = 600\n").unwrap();
        assert_eq!(config.shots.poll_ms, 5_000);
        assert_eq!(config.shots.offer_secs, 30);
    }

    #[test]
    fn unknown_sections_survive_and_are_reportable() {
        let config = Config::from_toml("[future]\nshiny = true\n").unwrap();
        assert_eq!(config.unknown_keys(), vec!["future"]);
    }

    #[test]
    fn token_is_thirty_two_hex_characters() {
        let token = generate_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(token, generate_token());
    }

    #[cfg(unix)]
    #[test]
    fn saved_config_is_owner_only() {
        let dir = std::env::temp_dir().join(format!("peekle-cfg-{}", Ulid::generate()));
        let path = dir.join("config.toml");
        Config::default().save(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, OWNER_ONLY);
        fs::remove_dir_all(&dir).unwrap();
    }
}
