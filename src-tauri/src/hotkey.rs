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

/// Which of the configured combinations fired.
///
/// The handler is one function for every shortcut the plugin holds, and since
/// 6.13 there are two of them. Reading the combination rather than assuming it
/// matters: the attach key is a bare arrow, and switching the whole product
/// off on it would be a bad surprise. tech.md 6.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Toggle,
    Attach,
}

pub fn role_of(app: &AppHandle, fired: &Shortcut) -> Option<Role> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    let (toggle, attach) = {
        let config = state.lock_config();
        (config.hotkey.toggle.clone(), config.hotkey.attach.clone())
    };

    if fires(&toggle, fired) {
        return Some(Role::Toggle);
    }
    if fires(&attach, fired) {
        return Some(Role::Attach);
    }
    None
}

/// Whether a config spelling names the combination that fired. An unparseable
/// spelling names nothing, which is the same answer `install` gives it.
fn fires(spelling: &str, fired: &Shortcut) -> bool {
    Combination::parse(spelling)
        .ok()
        .and_then(|combination| shortcut_of(&combination))
        .is_some_and(|shortcut| shortcut.matches(fired.mods, fired.key))
}

/// Takes one combination, by its config spelling.
///
/// Used by the screenshot offer, which holds its key for seconds rather than
/// for the life of the app: a modifierless key kept from the whole system
/// forever would be a fault. tech.md 6.13 and R-14.
pub fn register(app: &AppHandle, spelling: &str) -> Result<(), HotkeyError> {
    let combination = Combination::parse(spelling)?;
    PluginRegistrar::new(app.clone()).register(&combination)
}

/// Hands one combination back to the system. A key nobody holds is not an
/// error: the offer settles down more than one path and each of them releases.
pub fn unregister(app: &AppHandle, spelling: &str) {
    let Ok(combination) = Combination::parse(spelling) else {
        return;
    };
    let Some(shortcut) = shortcut_of(&combination) else {
        return;
    };
    if let Err(err) = app.global_shortcut().unregister(shortcut) {
        tracing::debug!(error = %err, "the attach key was not held");
    }
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
