//! Global shortcut registration. tech.md 6.9.
//!
//! The parsing and the trait live in `peekle-hotkey`; this is the half that
//! needs an `AppHandle`. A combination another application already holds is a
//! reported condition, not a crash: everything else keeps working.

use std::sync::Arc;

use peekle_hotkey::{Combination, HotkeyError, HotkeyRegistrar};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::state::AppState;

/// The real registrar, on Carbon RegisterEventHotKey under the plugin.
pub struct PluginRegistrar {
    app: AppHandle,
}

impl PluginRegistrar {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl HotkeyRegistrar for PluginRegistrar {
    fn register(&self, combination: &Combination) -> Result<(), HotkeyError> {
        let shortcut = shortcut_of(combination).ok_or(HotkeyError::Malformed)?;

        // The plugin does not tell a taken combination apart from any other
        // failure, and taken is the one worth naming: it is the case the user
        // can do something about. tech.md 6.9 and R-1.
        self.app
            .global_shortcut()
            .register(shortcut)
            .map_err(|err| {
                tracing::warn!(error = %err, "could not register the combination");
                HotkeyError::Taken
            })
    }

    fn unregister_all(&self) {
        let _ = self.app.global_shortcut().unregister_all();
    }
}

/// Maps our spelling onto the plugin's. An unknown key yields None rather than
/// a guess: registering the wrong key is worse than registering nothing.
pub fn shortcut_of(combination: &Combination) -> Option<Shortcut> {
    let mut modifiers = Modifiers::empty();
    if combination.alt {
        modifiers |= Modifiers::ALT;
    }
    if combination.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if combination.control {
        modifiers |= Modifiers::CONTROL;
    }
    if combination.command {
        modifiers |= Modifiers::SUPER;
    }

    let code: Code = combination.key.parse().ok()?;
    Some(Shortcut::new(Some(modifiers), code))
}

/// The shortcut a spelling maps to, or None if it does not parse. Used by the
/// handler to tell the two combinations apart. tech.md 6.9.
pub fn parse_shortcut(spelling: &str) -> Option<Shortcut> {
    Combination::parse(spelling)
        .ok()
        .as_ref()
        .and_then(shortcut_of)
}

/// Registers a combination. A failure is a flag and one warning, never a
/// reason not to start. tech.md 6.9.
pub fn install(app: &AppHandle, spelling: &str) {
    let state = app.state::<Arc<AppState>>().inner().clone();

    let combination = match Combination::parse(spelling) {
        Ok(combination) => combination,
        Err(err) => {
            tracing::warn!(error = %err, "the configured combination does not parse");
            state.set_hotkey_ok(false);
            crate::windows::warn_hotkey(app, "Hotkey is not a valid combination");
            return;
        }
    };

    if let Err(err) = PluginRegistrar::new(app.clone()).register(&combination) {
        tracing::warn!(error = %err, "the combination is unavailable");
        state.set_hotkey_ok(false);
        crate::windows::warn_hotkey(
            app,
            &format!("{} is taken by another app", combination.to_display()),
        );
        return;
    }

    tracing::info!(combination = %combination.to_display(), "combination registered");
}
