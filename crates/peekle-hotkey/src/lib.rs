//! Hotkey parsing and registration. tech.md section 6.9.
//!
//! Registration goes through tauri-plugin-global-shortcut, which sits on
//! Carbon RegisterEventHotKey. That buys three things the old double-`q` had
//! not: no Accessibility permission, system level interception so the key
//! never reaches the app in front, and no false fire while typing.

use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyError {
    #[error("the combination is empty")]
    Empty,
    #[error("the combination has no key, only modifiers")]
    NoKey,
    #[error("the combination is held by another application")]
    Taken,
    #[error("the combination could not be parsed")]
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combination {
    pub alt: bool,
    pub shift: bool,
    pub control: bool,
    pub command: bool,
    /// Key code in the plugin's own spelling, for example `KeyQ`.
    pub key: String,
}

impl Combination {
    /// Parses the config spelling, for example `Alt+Shift+KeyQ`.
    pub fn parse(text: &str) -> Result<Self, HotkeyError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(HotkeyError::Empty);
        }

        let mut combination = Combination {
            alt: false,
            shift: false,
            control: false,
            command: false,
            key: String::new(),
        };

        for part in trimmed.split('+') {
            let part = part.trim();
            if part.is_empty() {
                return Err(HotkeyError::Malformed);
            }
            match part.to_ascii_lowercase().as_str() {
                "alt" | "option" => combination.alt = true,
                "shift" => combination.shift = true,
                "control" | "ctrl" => combination.control = true,
                "command" | "cmd" | "super" | "meta" => combination.command = true,
                _ => {
                    if !combination.key.is_empty() {
                        return Err(HotkeyError::Malformed);
                    }
                    combination.key = part.to_string();
                }
            }
        }

        if combination.key.is_empty() {
            return Err(HotkeyError::NoKey);
        }
        Ok(combination)
    }

    /// Round trips back into the config spelling.
    pub fn to_config(&self) -> String {
        let mut parts = Vec::new();
        if self.control {
            parts.push("Control".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.command {
            parts.push("Command".to_string());
        }
        parts.push(self.key.clone());
        parts.join("+")
    }

    /// What the user sees in onboarding and in `doctor`, for example ⌥⇧Q.
    pub fn to_display(&self) -> String {
        let mut out = String::new();
        if self.control {
            out.push('⌃');
        }
        if self.alt {
            out.push('⌥');
        }
        if self.shift {
            out.push('⇧');
        }
        if self.command {
            out.push('⌘');
        }
        out.push_str(self.key.strip_prefix("Key").unwrap_or(&self.key));
        out
    }
}

pub trait HotkeyRegistrar: Send + Sync + 'static {
    fn register(&self, combination: &Combination) -> Result<(), HotkeyError>;
    fn unregister_all(&self);
}

/// Fake registrar. Records what it was asked for and can refuse, so the taken
/// combination path of section 6.9 has a test. tech.md section 7.
#[derive(Debug, Default)]
pub struct FakeHotkeyRegistrar {
    registered: Mutex<Vec<Combination>>,
    refuse: bool,
}

impl FakeHotkeyRegistrar {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registrar that always reports the combination as taken.
    pub fn taken() -> Self {
        Self {
            registered: Mutex::new(Vec::new()),
            refuse: true,
        }
    }

    pub fn registered(&self) -> Vec<Combination> {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Combination>> {
        self.registered.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl HotkeyRegistrar for FakeHotkeyRegistrar {
    fn register(&self, combination: &Combination) -> Result<(), HotkeyError> {
        if self.refuse {
            return Err(HotkeyError::Taken);
        }
        self.lock().push(combination.clone());
        Ok(())
    }

    fn unregister_all(&self) {
        self.lock().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_default_combination() {
        let parsed = Combination::parse("Alt+Shift+KeyQ").unwrap();
        assert!(parsed.alt && parsed.shift);
        assert!(!parsed.control && !parsed.command);
        assert_eq!(parsed.key, "KeyQ");
    }

    #[test]
    fn renders_for_onboarding_and_doctor() {
        assert_eq!(
            Combination::parse("Alt+Shift+KeyQ").unwrap().to_display(),
            "⌥⇧Q"
        );
        assert_eq!(
            Combination::parse("Control+Command+KeyK")
                .unwrap()
                .to_display(),
            "⌃⌘K"
        );
    }

    #[test]
    fn rejects_what_cannot_be_registered() {
        assert_eq!(Combination::parse(""), Err(HotkeyError::Empty));
        assert_eq!(Combination::parse("   "), Err(HotkeyError::Empty));
        assert_eq!(Combination::parse("Alt+Shift"), Err(HotkeyError::NoKey));
        assert_eq!(Combination::parse("Alt++KeyQ"), Err(HotkeyError::Malformed));
        assert_eq!(Combination::parse("KeyQ+KeyW"), Err(HotkeyError::Malformed));
    }

    #[test]
    fn a_taken_combination_is_reported_rather_than_swallowed() {
        let registrar = FakeHotkeyRegistrar::taken();
        let combination = Combination::parse("Alt+Shift+KeyQ").unwrap();

        assert_eq!(registrar.register(&combination), Err(HotkeyError::Taken));
        assert!(registrar.registered().is_empty());
    }

    #[test]
    fn a_free_combination_registers() {
        let registrar = FakeHotkeyRegistrar::new();
        let combination = Combination::parse("Alt+Shift+KeyQ").unwrap();

        assert_eq!(registrar.register(&combination), Ok(()));
        assert_eq!(registrar.registered(), vec![combination]);
    }
}
