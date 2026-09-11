//! The platform layer: what the rest of the app may ask of the window system,
//! the clipboard, the Trash and the app's front, and nothing about how any
//! platform answers. tech.md 6.27.
//!
//! macOS is the reference and keeps every word of 6.7, 6.13 and 6.26 in
//! `macos.rs`, moved there without a change. `desktop.rs` answers the same
//! surface for Windows and Linux through the cross-platform API of Tauri, and
//! says in its own comments what it cannot promise. The functions that are the
//! same everywhere -- the island's frame, its rectangle, whether it takes
//! clicks -- live here, once.

use peekle_core::island::Rect;
use peekle_core::types::IslandView;
use tauri::{AppHandle, LogicalPosition, Manager, WebviewWindow};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::active_screen;
#[cfg(target_os = "macos")]
pub use macos::{
    apply_view, convert_all, give_front_back, notch_for, show, take_front, to_trash, watch_pointer,
    SystemPasteboard,
};

#[cfg(not(target_os = "macos"))]
mod desktop;
#[cfg(not(target_os = "macos"))]
use desktop::active_screen;
#[cfg(not(target_os = "macos"))]
pub use desktop::{
    apply_view, convert_all, give_front_back, notch_for, show, take_front, to_trash, watch_pointer,
    SystemPasteboard,
};

pub const ISLAND: &str = "island";

/// The notch as measured on the display: height and width in points. None on
/// every platform but macOS, and on macOS on every display without one.
pub type Notch = (f64, f64);

#[derive(Debug, thiserror::Error)]
pub enum PanelError {
    #[error("window {0} is missing")]
    MissingWindow(String),
    /// Raised only where there is a panel to miss: the desktop has none and
    /// never constructs this, and the enum is one enum on every platform.
    #[allow(dead_code)]
    #[error("panel {0} is not registered")]
    MissingPanel(String),
    #[error("tauri call failed: {0}")]
    Tauri(#[from] tauri::Error),
}

/// The notch of the display the island opens on. tech.md 6.7 and 6.27.
pub fn active_notch(app: &AppHandle) -> Option<Notch> {
    notch_for(active_screen(app)?.1)
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

pub(crate) fn window(app: &AppHandle, label: &str) -> Result<WebviewWindow, PanelError> {
    app.get_webview_window(label)
        .ok_or_else(|| PanelError::MissingWindow(label.to_string()))
}

/// A collapsed island is a transparent 720 by 560 rectangle over the top of the
/// screen. Letting it take clicks would break everything under it, so mouse
/// events are switched by view and only by Rust. tech.md 6.7.
pub fn set_takes_clicks(app: &AppHandle, takes_clicks: bool) -> Result<(), PanelError> {
    window(app, ISLAND)?.set_ignore_cursor_events(!takes_clicks)?;
    tracing::debug!(takes_clicks, "island cursor events");
    Ok(())
}

/// Places the window per the geometry of section 6.7: top edge, centred on
/// the display the platform names. The size is never touched. tech.md 6.7.
pub(crate) fn position(app: &AppHandle, label: &str) -> Result<(), PanelError> {
    let window = window(app, label)?;

    let Some((origin, screen)) = active_screen(app) else {
        // No screen to place against. Leave the window where it is rather
        // than dropping it at the origin.
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

/// Whether a view has something to type into.
///
/// The desktop substitute for `becomes_key_only_if_needed`: a window that can
/// never take focus cannot be typed into, and one that always can steals the
/// keyboard from the app under it on every click. So it can while a view has a
/// field and cannot while it has only buttons: a click on Allow or Deny leaves
/// the front where it was, as it does on macOS, and a click into the field
/// takes it. Pure, and shared, so the rule is the same on every platform even
/// where only one of them acts on it. tech.md 6.27.
// Acted on by the desktop only, but compiled and tested everywhere: a rule
// with one home is a rule that cannot drift between platforms.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn focusable_for(view: &IslandView) -> bool {
    matches!(view, IslandView::Sessions | IslandView::Session(_))
}

/// A cheap signature of an image on the clipboard, for platforms with no
/// change counter of their own: its size and a hash of its first and last
/// bytes. Two different screenshots of the same size that agree on both ends
/// are the one collision this accepts. tech.md 6.27.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn image_signature(width: usize, height: usize, bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
    let edge = 4096.min(bytes.len());
    bytes[..edge].hash(&mut hasher);
    bytes[bytes.len() - edge..].hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Buttons alone never take the keyboard; a field does. tech.md 6.27.
    #[test]
    fn only_a_view_with_a_field_may_take_focus() {
        assert!(!focusable_for(&IslandView::Collapsed));
        assert!(!focusable_for(&IslandView::Pill));
        assert!(!focusable_for(&IslandView::Ask));
        assert!(focusable_for(&IslandView::Sessions));
        assert!(focusable_for(&IslandView::Session("s".into())));
    }

    /// The signature stands in for a change counter, so two reads of one
    /// picture must agree and a different picture must not.
    #[test]
    fn the_image_signature_tells_pictures_apart_and_repeats_itself() {
        let a = vec![7u8; 10_000];
        let mut b = a.clone();
        b[9_999] = 8;
        assert_eq!(image_signature(100, 25, &a), image_signature(100, 25, &a));
        assert_ne!(image_signature(100, 25, &a), image_signature(100, 25, &b));
        assert_ne!(image_signature(100, 25, &a), image_signature(25, 100, &a));
        assert_eq!(image_signature(0, 0, &[]), image_signature(0, 0, &[]));
    }
}
