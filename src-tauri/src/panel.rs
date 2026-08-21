//! All AppKit lives here. tech.md section 6.7 is the window contract and every
//! flag below is spelled out there, including the traps that produce no error
//! when you get them wrong.

use peekle_core::island::Rect;
use tauri::{AppHandle, LogicalPosition, Manager, WebviewWindow};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, Panel, PanelLevel, StyleMask, WebviewWindowExt,
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

    // No `.borderless()` here, and that is the whole of R-11. The builder in
    // tauri-nspanel assigns rather than ors:
    //
    //     pub fn borderless(mut self) -> Self { self.0 = Borderless; self }
    //
    // Borderless is zero, so chaining it wiped the NonactivatingPanel bit set
    // the line before and left the mask at 0. macOS will not place a window
    // without that bit on another application's full screen space, which is
    // the one promise this product is built on. Borderless adds nothing anyway:
    // it is the absence of the other bits.
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());
    panel.set_hides_on_deactivate(false);
    panel.set_released_when_closed(false);
    panel.set_has_shadow(false);
    panel.set_opaque(false);
    panel.set_becomes_key_only_if_needed(true);
    apply_space_behavior(panel.as_ref());

    // Nothing is expanded yet, so clicks pass through to whatever is below.
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

/// Join every space and survive over full screen video, above everything else.
///
/// Re-asserted on every show rather than set once: this is the one promise the
/// product cannot degrade on, and AppKit resets collection behavior on frame
/// view changes. Cheap to repeat, expensive to get silently wrong. tech.md 6.7.
fn apply_space_behavior(panel: &dyn Panel) {
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            // Stationary keeps the panel out of the Spaces animation, and
            // ignores_cycle keeps it out of window cycling. Neither is enough
            // on its own to reach another application's full screen space:
            // see R-11.
            .stationary()
            .ignores_cycle()
            .value(),
    );
    panel.set_level(PanelLevel::ScreenSaver.value());
}

/// Reads the behavior back off the live NSWindow. Setting it is not proof it
/// stuck: AppKit drops it on some frame view changes, and the only symptom is
/// an overlay that quietly never reaches a full screen space.
fn trace_space_behavior(window: &WebviewWindow) {
    use objc2_app_kit::NSWindow;
    use tauri_nspanel::objc2::msg_send;

    let Ok(handle) = window.ns_window() else {
        return;
    };
    let ns: &NSWindow = unsafe { &*(handle as *const NSWindow) };
    let behavior: usize = unsafe { msg_send![ns, collectionBehavior] };
    let level: isize = unsafe { msg_send![ns, level] };

    // 1 is canJoinAllSpaces, 256 is fullScreenAuxiliary. tech.md 6.7.
    tracing::debug!(
        behavior,
        level,
        joins_all_spaces = behavior & 1 != 0,
        full_screen_auxiliary = behavior & 256 != 0,
        "island space behavior"
    );
}

/// The notch of the display with the given logical size, or None when that
/// display has none.
///
/// Measured rather than guessed: the value differs per model, and a hardcoded
/// one puts the island either inside the bezel or floating below it. The
/// height is the top safe area inset; the width is what the menu bar cannot
/// use, which is the screen minus both auxiliary areas. tech.md 6.7.
///
/// Screens are matched by size rather than by origin: AppKit counts y upwards
/// from the primary display and Tauri counts it downwards, and a mismatch there
/// would silently pick the wrong screen. A wrong match here costs a floating
/// pill instead of one hugging the bezel, never a misplaced window.
pub fn notch_for(size: (f64, f64)) -> Option<Notch> {
    use objc2_app_kit::NSScreen;

    let mtm = objc2_foundation::MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);

    let screen = screens.iter().find(|screen| {
        let frame = screen.frame().size;
        (frame.width - size.0).abs() < 2.0 && (frame.height - size.1).abs() < 2.0
    })?;

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

/// The notch of the display the pointer is on. tech.md 6.7.
pub fn active_notch(app: &AppHandle) -> Option<Notch> {
    notch_for(active_screen(app)?.1)
}

/// The display the user is on, as logical origin and size.
///
/// The pointer is the signal: a non-activating overlay never owns the key
/// window, and the hand is where the eyes are. tech.md 6.7.
fn active_screen(app: &AppHandle) -> Option<((f64, f64), (f64, f64))> {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|point| app.monitor_from_point(point.x, point.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())?;

    let scale = monitor.scale_factor();
    let origin = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    Some(((origin.x, origin.y), (size.width, size.height)))
}

/// The island frame in physical pixels, with the scale the webview draws at.
///
/// Physical on purpose: the pointer arrives from Tauri in physical pixels and
/// displays can differ in scale, so converting one of them into the logical
/// space of the other is where an off by a factor of two would hide.
pub fn island_frame(app: &AppHandle) -> Result<(Rect, f64), PanelError> {
    let window = window(app, ISLAND)?;
    let position = window.outer_position()?;
    let size = window.outer_size()?;

    Ok((
        Rect::new(
            f64::from(position.x),
            f64::from(position.y),
            f64::from(size.width),
            f64::from(size.height),
        ),
        window.scale_factor()?,
    ))
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

    apply_space_behavior(panel.as_ref());
    panel.order_front_regardless();
    trace_space_behavior(&window(app, label)?);
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

    let Some((origin, screen)) = active_screen(app) else {
        // No screen to place against. Leave the panel where it is rather than
        // dropping it at the origin.
        return Ok(());
    };
    let scale = window
        .current_monitor()?
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    let size = window.outer_size()?.to_logical::<f64>(scale);

    // Flush with the top edge so the black fill continues the notch. A display
    // without one gets its inset from the shape, not from the window.
    let x = (screen.0 - size.width) / 2.0;
    window.set_position(LogicalPosition::new(origin.0 + x, origin.1))?;

    tracing::debug!(
        label,
        scale,
        screen_w = screen.0,
        screen_h = screen.1,
        win_w = size.width,
        win_h = size.height,
        placed_x = origin.0 + x,
        placed_y = origin.1,
        "panel placed"
    );
    Ok(())
}
