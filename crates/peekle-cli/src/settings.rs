//! `~/.claude/settings.json` access. tech.md sections 6.1, 7 and 11.
//!
//! That file is the user's territory. Peekle merges, never replaces, backs it
//! up with a timestamp before every write, and removes only entries it can
//! prove are its own.

use std::io;
use std::path::{Path, PathBuf};

use peekle_core::hooks::{
    hook_script_path, HOOK_PATH_PREFIX, HOOK_SCRIPT_NAME, MANAGED_HOOK_EVENTS,
};
use serde_json::{json, Map, Value};

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("settings io failed: {0}")]
    Io(#[from] io::Error),
    #[error("settings file is not valid json: {0}")]
    Parse(#[from] serde_json::Error),
}

pub trait ClaudeSettings: Send + Sync {
    fn read(&self) -> Result<Value, SettingsError>;
    /// Writes a timestamped backup next to the file, then the new contents.
    fn write(&self, value: &Value) -> Result<PathBuf, SettingsError>;
}

pub struct FileSettings {
    path: PathBuf,
}

impl FileSettings {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".claude/settings.json"))
    }
}

impl ClaudeSettings for FileSettings {
    fn read(&self) -> Result<Value, SettingsError> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) if text.trim().is_empty() => Ok(json!({})),
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(json!({})),
            Err(err) => Err(SettingsError::Io(err)),
        }
    }

    fn write(&self, value: &Value) -> Result<PathBuf, SettingsError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let backup = backup_path(&self.path);
        if self.path.exists() {
            std::fs::copy(&self.path, &backup)?;
        }
        std::fs::write(&self.path, serde_json::to_string_pretty(value)?)?;
        Ok(backup)
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".peekle-bak.{stamp}"));
    PathBuf::from(name)
}

/// Fake backed by a temp directory. Every `peekle init` test runs on this, so
/// no test ever touches the real settings file. tech.md section 7.
pub struct FakeSettings {
    inner: FileSettings,
}

impl FakeSettings {
    pub fn in_dir(dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self {
            inner: FileSettings::new(dir.join("settings.json")),
        })
    }

    pub fn seed(&self, value: &Value) -> Result<(), SettingsError> {
        self.inner.write(value)?;
        Ok(())
    }
}

impl ClaudeSettings for FakeSettings {
    fn read(&self) -> Result<Value, SettingsError> {
        self.inner.read()
    }

    fn write(&self, value: &Value) -> Result<PathBuf, SettingsError> {
        self.inner.write(value)
    }
}

/// True when a handler entry is one Peekle wrote: loopback host on our port,
/// path under `/v1/h/`. Anything else belongs to the user and is left alone.
pub fn is_peekle_handler(entry: &Value, port: u16) -> bool {
    // A command handler is ours when it runs our script. The url arm keeps
    // recognising what older versions wrote, so an upgrade replaces those
    // rather than leaving a second handler behind. Both stay as narrow as they
    // were: another Peekle on another port is somebody else's. tech.md 6.1.
    if let Some(command) = entry.get("command").and_then(Value::as_str) {
        return command.contains(HOOK_SCRIPT_NAME);
    }
    entry
        .get("url")
        .and_then(Value::as_str)
        .is_some_and(|url| url.starts_with(&format!("http://127.0.0.1:{port}{HOOK_PATH_PREFIX}")))
}

/// The hook script, compiled in so `init` never depends on where the binary
/// was unpacked from. tech.md 6.1.
const HOOK_SCRIPT: &str = include_str!("../../../scripts/peekle-hook.py");

/// Writes the hook script and makes it executable.
///
/// Rewritten on every `init` rather than only when missing: the script ships
/// with the binary, so an upgraded Peekle with an older script on disk would
/// speak a protocol nobody is listening to.
pub fn install_hook_script() -> io::Result<PathBuf> {
    let path = hook_script_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, HOOK_SCRIPT)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(path)
}

/// Merges Peekle's handlers into the settings tree, replacing only its own
/// entries. Idempotent: running it twice changes nothing.
pub fn merge_handlers(settings: &Value, port: u16, _token: &str) -> Value {
    let mut root = settings.as_object().cloned().unwrap_or_default();
    let mut hooks = root
        .get("hooks")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for event in MANAGED_HOOK_EVENTS {
        let mut ours = json!({
            "type": "command",
            "command": hook_command(),
        });
        if let Some(timeout) = timeout_for(event) {
            ours["timeout"] = json!(timeout);
        }

        let mut matchers = hooks
            .get(*event)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        strip_ours(&mut matchers, port);
        matchers.push(matcher_entry(event, ours));
        hooks.insert((*event).to_string(), Value::Array(matchers));
    }

    root.insert("hooks".to_string(), Value::Object(hooks));
    Value::Object(root)
}

/// Removes Peekle's handlers and nothing else. Drops the `hooks` key when it
/// is left empty.
pub fn remove_handlers(settings: &Value, port: u16) -> Value {
    let mut root = settings.as_object().cloned().unwrap_or_default();
    let Some(mut hooks) = root.get("hooks").and_then(Value::as_object).cloned() else {
        return Value::Object(root);
    };

    for event in MANAGED_HOOK_EVENTS {
        let Some(mut matchers) = hooks.get(*event).and_then(Value::as_array).cloned() else {
            continue;
        };
        strip_ours(&mut matchers, port);
        if matchers.is_empty() {
            hooks.remove(*event);
        } else {
            hooks.insert((*event).to_string(), Value::Array(matchers));
        }
    }

    if hooks.is_empty() {
        root.remove("hooks");
    } else {
        root.insert("hooks".to_string(), Value::Object(hooks));
    }
    Value::Object(root)
}

fn strip_ours(matchers: &mut Vec<Value>, port: u16) {
    matchers.retain_mut(|matcher| {
        let Some(list) = matcher.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        list.retain(|entry| !is_peekle_handler(entry, port));
        !list.is_empty()
    });
}

fn matcher_entry(event: &str, handler: Value) -> Value {
    let mut entry = Map::new();
    if let Some(matcher) = matcher_for(event) {
        entry.insert("matcher".to_string(), json!(matcher));
    }
    entry.insert("hooks".to_string(), json!([handler]));
    Value::Object(entry)
}

/// tech.md 6.1, the matcher column.
fn matcher_for(event: &str) -> Option<&'static str> {
    match event {
        "PermissionRequest" | "PreToolUse" | "PostToolUse" => Some("*"),
        "Notification" => Some("permission_prompt|idle_prompt|agent_needs_input|agent_completed"),
        _ => None,
    }
}

/// tech.md 6.1, the endpoint column, literally.
///
/// Every feed event goes to `/feed`. It went to `/session` and to a `/tasks`
/// that the router has never had, so every tool call in the product's life was
/// posted to a handler that drops it or to a 404: the live feed was dead while
/// its tests passed against the router directly.
pub fn endpoint_for(event: &str) -> &'static str {
    match event {
        "Stop" => "stop",
        "PermissionRequest" => "permission",
        "Notification" => "notification",
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => "feed",
        _ => "session",
    }
}

/// Only the hooks that can wait for a person carry a timeout, and it is an
/// upper bound rather than a working window: Peekle gives up first, on its own
/// terms, using the windows in 6.8. A whole day because the one waiting is a
/// process, not the user. tech.md 6.1.
fn timeout_for(event: &str) -> Option<u32> {
    match event {
        "Stop" | "PermissionRequest" => Some(86400),
        _ => None,
    }
}

/// The command `init` installs. Absolute python3 rather than a bare name: the
/// hook runs with whatever environment Claude Code had, which is not a login
/// shell. tech.md 6.1.
fn hook_command() -> String {
    format!("/usr/bin/env python3 {}", hook_script_path().display())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every managed event, pinned to the endpoint and matcher of the table in
    /// tech.md 6.1. The mapping drifted from that table and nothing noticed,
    /// because every other test posts to the router directly and never reads
    /// what `init` actually wrote. This one reads it.
    #[test]
    fn every_event_lands_on_the_endpoint_the_contract_names() {
        for (event, endpoint, matcher) in [
            ("Stop", "stop", None),
            ("PermissionRequest", "permission", Some("*")),
            ("UserPromptSubmit", "feed", None),
            ("PreToolUse", "feed", Some("*")),
            ("PostToolUse", "feed", Some("*")),
            (
                "Notification",
                "notification",
                Some("permission_prompt|idle_prompt|agent_needs_input|agent_completed"),
            ),
            ("SessionStart", "session", None),
            ("SessionEnd", "session", None),
        ] {
            assert_eq!(endpoint_for(event), endpoint, "{event}");
            assert_eq!(matcher_for(event), matcher, "{event}");
        }
    }

    /// A route that does not exist answers 404, and a hook posting into one is
    /// invisible in every log the user can reach.
    #[test]
    fn no_event_is_sent_to_a_route_the_server_does_not_serve() {
        const SERVED: &[&str] = &["stop", "permission", "notification", "feed", "session"];
        for event in peekle_core::MANAGED_HOOK_EVENTS {
            assert!(SERVED.contains(&endpoint_for(event)), "{event}");
        }
    }

    const PORT: u16 = 47821;
    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn foreign() -> Value {
        json!({
            "model": "opus",
            "hooks": {
                "Stop": [{"hooks": [{"type": "command", "command": "say done"}]}],
                "PreToolUse": [{"hooks": [{"type": "command", "command": "audit"}]}]
            }
        })
    }

    #[test]
    fn merging_keeps_foreign_handlers_and_foreign_keys() {
        let merged = merge_handlers(&foreign(), PORT, TOKEN);

        assert_eq!(merged["model"], "opus");
        assert_eq!(
            merged["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "audit"
        );
        assert_eq!(
            merged["hooks"]["Stop"][0]["hooks"][0]["command"],
            "say done"
        );
    }

    #[test]
    fn merging_is_idempotent() {
        let once = merge_handlers(&foreign(), PORT, TOKEN);
        let twice = merge_handlers(&once, PORT, TOKEN);
        assert_eq!(once, twice);
    }

    #[test]
    fn every_managed_event_gets_a_handler_with_its_matcher() {
        let merged = merge_handlers(&json!({}), PORT, TOKEN);

        for event in MANAGED_HOOK_EVENTS {
            let entries = merged["hooks"][*event].as_array().expect(event);
            let ours = entries.last().expect("a handler");
            assert!(is_peekle_handler(&ours["hooks"][0], PORT), "{event}");
            match matcher_for(event) {
                Some(matcher) => assert_eq!(ours["matcher"], matcher, "{event}"),
                None => assert!(ours.get("matcher").is_none(), "{event}"),
            }
        }
    }

    /// Only the two hooks that can wait for a person carry a timeout, and it
    /// is a ceiling rather than a working window: Peekle gives up first, on
    /// the windows in 6.8. A day, because the one waiting is a process.
    #[test]
    fn only_the_hooks_that_wait_for_a_person_carry_a_timeout() {
        let merged = merge_handlers(&json!({}), PORT, TOKEN);
        for event in ["Stop", "PermissionRequest"] {
            assert_eq!(
                merged["hooks"][event][0]["hooks"][0]["timeout"], 86400,
                "{event}"
            );
        }
        for event in ["Notification", "PreToolUse", "SessionEnd"] {
            assert!(
                merged["hooks"][event][0]["hooks"][0]
                    .get("timeout")
                    .is_none(),
                "{event} does not wait for anybody"
            );
        }
    }

    /// Every handler runs the script, because only a child of the agent can
    /// report its pid and tty. tech.md 6.1.
    #[test]
    fn every_managed_event_runs_the_hook_script() {
        let merged = merge_handlers(&json!({}), PORT, TOKEN);
        for event in MANAGED_HOOK_EVENTS {
            let handler = &merged["hooks"][*event][0]["hooks"][0];
            assert_eq!(handler["type"], "command", "{event}");
            assert!(
                handler["command"]
                    .as_str()
                    .is_some_and(|c| c.contains(HOOK_SCRIPT_NAME)),
                "{event}"
            );
            assert!(handler.get("url").is_none(), "{event} is not http any more");
        }
    }

    /// An upgrade from the http era replaces those handlers rather than
    /// leaving a second one behind that posts to the same server.
    #[test]
    fn installing_over_the_old_http_handlers_replaces_them() {
        let old = json!({"hooks": {"Stop": [{"hooks": [
            {"type": "http", "url": format!("http://127.0.0.1:{PORT}/v1/h/{TOKEN}/stop")}
        ]}]}});
        let merged = merge_handlers(&old, PORT, TOKEN);
        let handlers = merged["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(handlers.len(), 1, "the old handler is gone");
        assert_eq!(handlers[0]["hooks"][0]["type"], "command");
    }

    #[test]
    fn uninstall_removes_only_our_entries() {
        let merged = merge_handlers(&foreign(), PORT, TOKEN);
        let cleaned = remove_handlers(&merged, PORT);

        assert_eq!(
            cleaned["hooks"]["Stop"][0]["hooks"][0]["command"],
            "say done"
        );
        assert_eq!(
            cleaned["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "audit"
        );
        assert!(cleaned["hooks"].get("PermissionRequest").is_none());
        assert_eq!(cleaned["model"], "opus");
    }

    #[test]
    fn uninstall_drops_the_hooks_key_once_it_is_empty() {
        let merged = merge_handlers(&json!({}), PORT, TOKEN);
        let cleaned = remove_handlers(&merged, PORT);
        assert!(cleaned.get("hooks").is_none());
    }

    #[test]
    fn a_handler_on_another_port_is_not_ours() {
        let entry = json!({"url": format!("http://127.0.0.1:9999/v1/h/{TOKEN}/stop")});
        assert!(!is_peekle_handler(&entry, PORT));
    }

    #[test]
    fn writing_leaves_a_timestamped_backup() {
        let dir = std::env::temp_dir().join(format!("peekle-settings-{}", ulid::Ulid::generate()));
        let settings = FakeSettings::in_dir(&dir).unwrap();
        settings.seed(&foreign()).unwrap();

        let backup = settings
            .write(&merge_handlers(&foreign(), PORT, TOKEN))
            .unwrap();
        assert!(backup.exists());

        let restored: Value =
            serde_json::from_str(&std::fs::read_to_string(&backup).unwrap()).unwrap();
        assert_eq!(restored["model"], "opus");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
