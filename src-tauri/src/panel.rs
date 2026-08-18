//! All AppKit lives here. tech.md section 6.7 is the window contract and every
//! flag below is spelled out there, including the traps that produce no error
//! when you get them wrong.

use tauri::{AppHandle, LogicalPosition, Manager, WebviewWindow};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

pub const ISLAND: &str = "island";

/// The notch as measured on the main display: height and width in points.
pub type Notch = (f64, f64);

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

/// The notch on the main display, or None when there is none.
///
/// Measured rather than guessed: the value differs per model, and a hardcoded
/// one puts the island either inside the bezel or floating below it. The
/// height is the top safe area inset; the width is what the menu bar cannot
/// use, which is the screen minus both auxiliary areas. tech.md 6.7.
pub fn notch() -> Option<Notch> {
    use objc2_app_kit::NSScreen;

    let mtm = objc2_foundation::MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;

    // A display without a notch reports a zero top inset. One with a notch
    // reports its height, which is what the island has to cover.
    let height = screen.safeAreaInsets().top;
    if height <= 0.0 {
        return None;
    }

    let left = screen.auxiliaryTopLeftArea().size.width;
    let right = screen.auxiliaryTopRightArea().size.width;
    let width = screen.frame().size.width - left - right;

    // No auxiliary areas means nothing to subtract from, and a width of the
    // whole screen would paint the entire top edge black.
    (width > 0.0 && width < screen.frame().size.width).then_some((height, width))
}

fn window(app: &AppHandle, label: &str) -> Result<WebviewWindow, PanelError> {
    app.get_webview_window(label)
        .ok_or_else(|| PanelError::MissingWindow(label.to_string()))
}

/// Places the panel per the geometry of section 6.7 and shows it. Position is
/// recomputed on every show because the user can move between displays.
///
/// The size is never touched. Resizing an NSWindow makes the system relayout
/// and repaint every frame, and with a webview inside that is a guaranteed
/// stutter, so the window sits at the bounds of the widest view and the shape
/// inside it does the moving. tech.md 6.7.
pub fn show(app: &AppHandle, label: &str) -> Result<(), PanelError> {
    position(app, label)?;
    let panel = app
        .get_webview_panel(label)
        .map_err(|_| PanelError::MissingPanel(label.to_string()))?;

    panel.order_front_regardless();
    tracing::debug!(label, visible = panel.is_visible(), "panel shown");
    Ok(())
}

/// A collapsed island is a transparent 720 by 560 rectangle over the top of the
/// screen. Letting it take clicks would break everything under it, so mouse
/// events are switched by view and only by Rust. tech.md 6.7.
pub fn set_takes_clicks(app: &AppHandle, takes_clicks: bool) -> Result<(), PanelError> {
    window(app, ISLAND)?.set_ignore_cursor_events(!takes_clicks)?;
    tracing::debug!(takes_clicks, "island cursor events");
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

    // Flush with the top edge so the black fill continues the notch. A display
    // without one gets its inset from the shape, not from the window.
    let x = (screen.width - size.width) / 2.0;
    let origin = monitor.position().to_logical::<f64>(scale);
    window.set_position(LogicalPosition::new(origin.x + x, origin.y))?;

    tracing::debug!(
        label,
        scale,
        screen_w = screen.width,
        screen_h = screen.height,
        win_w = size.width,
        win_h = size.height,
        placed_x = origin.x + x,
        placed_y = origin.y,
        "panel placed"
    );
    Ok(())
}
