//! Config file handling. tech.md section 6.8 is the source of truth for every
//! key and default. Unknown keys are kept out of the way and warned about, not
//! rejected: a newer Peekle must not brick an older config and the reverse.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// `~/Library/Application Support/peekle/config.toml`
pub const CONFIG_FILE: &str = "config.toml";
/// Port is duplicated here so `doctor` and `status` can find the server
/// without parsing the config.
pub const PORT_FILE: &str = ".peekle/port";

const OWNER_ONLY: u32 = 0o600;
const MAX_FEED_VISIBLE_ROWS: u8 = 6;

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
    /// Takes control of the session the island is showing, or gives it back.
    /// A hotkey and not only a button, because control has to be handed back
    /// at the moment the island is closed and the mouse cannot reach it.
    /// tech.md 6.9.
    pub takeover: String,
    /// Empty means do not register.
    pub recall: String,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageProviderKind {
    Account,
    Fake,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub enabled: bool,
    /// How long the permission hook waits for a decision. Held far below the
    /// hook timeout of 86400s so Peekle gives up first and hands the question
    /// back to the terminal. tech.md 6.8.
    pub permission_wait_secs: u32,
    /// How long a held turn waits for something to be typed. Expiring ends the
    /// pause, never the channel: text typed later rides the next Stop, so this
    /// has no lower bound worth warning about. tech.md 6.8.
    pub reply_window_secs: u32,
    /// How long a sent reply waits for `UserPromptSubmit` before it is called
    /// undelivered. tech.md 6.3.
    pub delivery_confirm_secs: u32,
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
            takeover: "Alt+Shift+KeyS".to_string(),
            recall: String::new(),
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
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            permission_wait_secs: 300,
            reply_window_secs: 300,
            delivery_confirm_secs: 20,
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
    /// the mode is part of the contract, not hygiene.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text)?;
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
        assert_eq!(config.hotkey.takeover, "Alt+Shift+KeyS");
        assert_eq!(config.behavior.permission_wait_secs, 300);
        assert_eq!(config.behavior.reply_window_secs, 300);
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let config = Config::from_toml("[server]\nport = 5000\ntoken = \"abc\"\n").unwrap();
        assert_eq!(config.server.port, 5000);
        assert_eq!(config.ui.feed_visible_rows, 6);
        assert!(config.usage.enabled);
    }

    #[test]
    fn partial_section_keeps_the_other_keys_of_that_section() {
        let config = Config::from_toml("[server]\nport = 5000\n").unwrap();
        assert_eq!(config.server.port, 5000);
        assert_eq!(config.server.token.len(), 32);

        let config = Config::from_toml("[behavior]\nenabled = false\n").unwrap();
        assert!(!config.behavior.enabled);
        assert_eq!(config.behavior.permission_wait_secs, 300);
        assert_eq!(config.behavior.reply_window_secs, 300);
    }

    #[test]
    fn feed_visible_rows_clamps_to_six() {
        let config = Config::from_toml("[ui]\nfeed_visible_rows = 40\n").unwrap();
        assert_eq!(config.ui.feed_visible_rows, 6);
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
