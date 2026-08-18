//! All AppKit lives here. tech.md section 6.7 is the window contract and every
//! flag below is spelled out there, including the traps that produce no error
//! when you get them wrong.

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

pub const ISLAND: &str = "island";

/// The island covers the notch and grows it by this much. tech.md 6.7.
const ISLAND_WIDTH: f64 = 420.0;
const ISLAND_BAND: f64 = 43.0;
/// Fallback offset on a display with no notch, where there is nothing to grow.
const ISLAND_TOP_INSET: f64 = 8.0;

// The island takes keystrokes without activating the app, so the user answers
// the agent while the menu bar still belongs to whatever they were watching.
// `becomes_key_only_if_needed` is what makes that selective: a click on a
// button stays passive, a click in a text field takes focus. tech.md 6.7.
tauri_panel! {
    panel!(IslandPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false
        }
    })
}

#[derive(Debug, thiserror::Error)]
pub enum PanelError {
    #[error("window {0} is missing")]
    MissingWindow(String),
    #[error("panel {0} is not registered")]
    MissingPanel(String),
    #[error("tauri call failed: {0}")]
    Tauri(#[from] tauri::Error),
}

/// Converts the one window into a panel. Runs once at startup: the panel is
/// shown and hidden afterwards, never created and destroyed.
pub fn convert_all(app: &AppHandle) -> Result<(), PanelError> {
    let window = window(app, ISLAND)?;
    let panel = window.to_panel::<IslandPanel>()?;

    // Join every space, survive over full screen video, and never hide when the
    // app deactivates. For an overlay that never activates, the app is
    // deactivated permanently, so the AppKit default of hiding on deactivate
    // would hide the panel forever.
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    panel.set_hides_on_deactivate(false);
    panel.set_released_when_closed(false);
    panel.set_has_shadow(false);
    panel.set_opaque(false);
    panel.set_style_mask(
        StyleMask::empty()
            .nonactivating_panel()
            .borderless()
            .value(),
    );
    panel.set_level(PanelLevel::ScreenSaver.value());
    panel.set_becomes_key_only_if_needed(true);

    // Nothing is expanded yet, so clicks pass through to whatever is below.
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

/// Height of the notch on the main display, or None when there is none.
///
/// Measured rather than guessed: the value differs per model, and a hardcoded
/// one puts the island either inside the bezel or floating below it.
/// tech.md 6.7.
pub fn notch_height() -> Option<f64> {
    use objc2_app_kit::NSScreen;

    let mtm = objc2_foundation::MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;

    // A display without a notch reports a zero top inset. One with a notch
    // reports its height, which is what the island has to cover.
    let top = screen.safeAreaInsets().top;
    (top > 0.0).then_some(top)
}

fn window(app: &AppHandle, label: &str) -> Result<WebviewWindow, PanelError> {
    app.get_webview_window(label)
        .ok_or_else(|| PanelError::MissingWindow(label.to_string()))
}

/// Places the panel per the geometry of section 6.7 and shows it. Position is
/// recomputed on every show because the user can move between displays.
pub fn show(app: &AppHandle, label: &str) -> Result<(), PanelError> {
    size_island(app)?;
    position(app, label)?;
    let panel = app
        .get_webview_panel(label)
        .map_err(|_| PanelError::MissingPanel(label.to_string()))?;

    panel.order_front_regardless();
    tracing::debug!(label, visible = panel.is_visible(), "panel shown");
    Ok(())
}

pub fn hide(app: &AppHandle, label: &str) -> Result<(), PanelError> {
    let panel = app
        .get_webview_panel(label)
        .map_err(|_| PanelError::MissingPanel(label.to_string()))?;
    panel.hide();
    tracing::debug!(label, visible = panel.is_visible(), "panel hidden");
    Ok(())
}

/// The island is as tall as the notch plus the band that carries the content.
fn size_island(app: &AppHandle) -> Result<(), PanelError> {
    let notch = notch_height().unwrap_or(0.0);
    window(app, ISLAND)?.set_size(LogicalSize::new(ISLAND_WIDTH, notch + ISLAND_BAND))?;
    Ok(())
}

fn position(app: &AppHandle, label: &str) -> Result<(), PanelError> {
    let window = window(app, label)?;

    let Some(monitor) = window.current_monitor()? else {
        // No monitor means no screen to place against. Leave the panel where
        // it is rather than dropping it at the origin.
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let screen = monitor.size().to_logical::<f64>(scale);
    let size = window.outer_size()?.to_logical::<f64>(scale);

    // Flush with the top edge so the black fill continues the notch. With no
    // notch there is nothing to continue, so it floats instead.
    let x = (screen.width - size.width) / 2.0;
    let y = if notch_height().is_some() {
        0.0
    } else {
        ISLAND_TOP_INSET
    };

    let origin = monitor.position().to_logical::<f64>(scale);
    window.set_position(LogicalPosition::new(origin.x + x, origin.y + y))?;

    tracing::debug!(
        label,
        scale,
        screen_w = screen.width,
        screen_h = screen.height,
        win_w = size.width,
        win_h = size.height,
        placed_x = origin.x + x,
        placed_y = origin.y + y,
        "panel placed"
    );
    Ok(())
}
