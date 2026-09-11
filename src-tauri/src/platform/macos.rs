//! All AppKit lives here, and nothing but AppKit: the panel, the pasteboard,
//! the Trash, the app's front. tech.md section 6.7 is the window contract and
//! every flag below is spelled out there, including the traps that produce
//! no error when you get them wrong. `mod.rs` is the surface the rest of the
//! app sees; `desktop.rs` is the same surface for Windows and Linux.
//! tech.md 6.27.

use std::path::Path;

use peekle_core::shots::Pasteboard;
use peekle_core::types::IslandView;
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, Panel, PanelLevel, StyleMask, WebviewWindowExt,
};

use super::{position, window, Notch, PanelError, ISLAND};

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
    // reports the height of the cutout, which is not quite the height of the
    // black band: the strip the system reserves for the menu bar is a point
    // taller here (34 against 33), and an island as tall as the cutout ends a
    // pixel above the menu bar with a seam of wallpaper under it. The eye
    // reads the band, so the taller of the two wins. A hidden menu bar zeroes
    // the second and leaves the cutout, which the first still describes.
    // tech.md 6.7.
    let inset = screen.safeAreaInsets().top;
    if inset <= 0.0 {
        return None;
    }
    let frame = screen.frame().size.height;
    let visible = screen.visibleFrame();
    let menu_bar = frame - visible.size.height - visible.origin.y;
    let height = inset.max(menu_bar);

    let left = screen.auxiliaryTopLeftArea().size.width;
    let right = screen.auxiliaryTopRightArea().size.width;
    let width = screen.frame().size.width - left - right;

    // No auxiliary areas means nothing to subtract from, and a width of the
    // whole screen would paint the entire top edge black.
    (width > 0.0 && width < screen.frame().size.width).then_some((height, width))
}

/// Calls back whenever the pointer moves anywhere on screen.
///
/// The resting island ignores the cursor, so it never sees a `mousemove` of
/// its own, and a poll is always one tick behind: a click landing inside that
/// tick goes through to the menu bar, and the mark reads as a button that
/// needs pressing twice. A pointer cannot reach the mark without moving, and
/// movement is an event. tech.md 6.7.
///
/// Global means events that went to other applications, which is every move
/// over a resting island. The monitor is installed once and lives as long as
/// the app: dropping the returned object removes it. No Accessibility grant is
/// involved -- that is required for keyboard events, not for mouse ones.
pub fn watch_pointer<F>(app: &AppHandle, moved: F) -> Result<(), String>
where
    F: Fn(&AppHandle) + 'static,
{
    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask};

    let handle = app.clone();
    let block = RcBlock::new(move |_event: core::ptr::NonNull<NSEvent>| {
        moved(&handle);
    });

    let monitor =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::MouseMoved, &block);
    match monitor {
        // Held for the life of the process on purpose: the island watches the
        // pointer for as long as it is on screen, which is always.
        Some(monitor) => {
            std::mem::forget(monitor);
            Ok(())
        }
        None => Err("the system refused a mouse monitor".to_string()),
    }
}

/// The display the user is on, as logical origin and size.
///
/// The pointer is the signal: a non-activating overlay never owns the key
/// window, and the hand is where the eyes are. tech.md 6.7.
pub(super) fn active_screen(app: &AppHandle) -> Option<((f64, f64), (f64, f64))> {
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

/// Brings the app to the front for the length of a system dialog.
///
/// The one place the product takes focus on purpose. A file dialog comes up on
/// the active application, and this one is an accessory with no window of its
/// own to activate through, so it has to say so. tech.md 6.23.
pub fn take_front() {
    use objc2_app_kit::NSApplication;
    use tauri_nspanel::objc2::MainThreadMarker;

    let Some(marker) = MainThreadMarker::new() else {
        // Off the main thread there is nothing safe to do with AppKit, and a
        // dialog behind another window is better than a crash.
        tracing::warn!("not on the main thread, leaving the front alone");
        return;
    };
    // `activateIgnoringOtherApps:` is deprecated since macOS 14 and does
    // nothing there; this is the call that still means what it says.
    NSApplication::sharedApplication(marker).activate();
}

/// Gives the front back to whoever had it. tech.md 6.23.
pub fn give_front_back() {
    use objc2_app_kit::NSApplication;
    use tauri_nspanel::objc2::MainThreadMarker;

    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    NSApplication::sharedApplication(marker).deactivate();
}

/// The view changed. Nothing to do here: the panel decides key status per
/// click through `becomes_key_only_if_needed`, and that is the whole point of
/// it being a panel. Desktop has to switch focusability by view instead.
/// tech.md 6.27.
pub fn apply_view(_app: &AppHandle, _view: &IslandView) {}

/// The real pasteboard. tech.md 6.13.
///
/// Two halves, deliberately unequal. Detection reads the change count and the
/// type names, which raises nothing and copies nothing. Reading the contents
/// runs once, after the user pressed the key: from macOS 15 that read is a
/// system paste prompt, and asking for one on every copy anybody makes would
/// be a product that spies. tech.md R-13.
#[derive(Default)]
pub struct SystemPasteboard;

impl SystemPasteboard {
    pub fn new() -> Self {
        Self
    }
}

impl Pasteboard for SystemPasteboard {
    fn change_count(&self) -> i64 {
        use objc2_app_kit::NSPasteboard;

        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.changeCount() as i64
    }

    fn item_types(&self) -> Vec<Vec<String>> {
        use objc2_app_kit::NSPasteboard;

        let pasteboard = NSPasteboard::generalPasteboard();
        // The item and not the pasteboard: NSPasteboard synthesises TIFF from
        // a PNG, so its declared list calls every screenshot an image and
        // every image a screenshot. tech.md 6.13.
        let Some(items) = pasteboard.pasteboardItems() else {
            return Vec::new();
        };
        items
            .iter()
            .map(|item| item.types().iter().map(|t| t.to_string()).collect())
            .collect()
    }

    fn read_png(&self) -> Option<Vec<u8>> {
        use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG};

        let pasteboard = NSPasteboard::generalPasteboard();
        // The extern static is the type name itself, and reading one is what
        // `unsafe` covers here. Nothing about the read is fallible otherwise.
        let png = unsafe { NSPasteboardTypePNG };
        let data = pasteboard.dataForType(png)?;
        Some(data.to_vec())
    }
}

/// Moves one file to the Trash. tech.md 6.26.
///
/// `NSFileManager.trashItemAtURL` and never `std::fs::remove_file`: a
/// transcript is a person's own conversation, and "this chat is gone" is a
/// decision they must be able to take back the way they take back every other
/// deletion on this machine -- in Finder. It is also what Finder itself calls,
/// so the file lands where the user looks for it. A refusal from the system is
/// an error and never a silent success.
pub fn to_trash(path: &Path) -> Result<(), String> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    let text = path.to_string_lossy().to_string();
    let url = NSURL::fileURLWithPath(&NSString::from_str(&text));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|err| err.localizedDescription().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ignored on purpose: it puts a real file in the real Trash, which is
    /// exactly what makes it worth running by hand (`cargo test -- --ignored
    /// goes_to_the_trash`) and exactly what a suite must not do on its own.
    /// It cleans up after itself, and only after the one file it created.
    #[test]
    #[ignore = "touches the user's Trash"]
    fn a_file_goes_to_the_trash_and_not_to_nowhere() {
        let name = format!("peekle-trash-probe-{}.txt", std::process::id());
        let path = std::env::temp_dir().join(&name);
        std::fs::write(&path, b"probe").expect("wrote the probe");

        to_trash(&path).expect("the system took the file");
        assert!(!path.exists(), "the file left where it was");

        let home = std::env::var("HOME").expect("a home");
        let landed = std::path::Path::new(&home).join(".Trash").join(&name);
        assert!(landed.exists(), "the file is in the Trash");
        std::fs::remove_file(&landed).expect("cleaned up the probe");
    }

    /// A path that is not there is the system's answer to give, and it says no.
    /// Deletion must never report a success it did not have. tech.md 6.26.
    #[test]
    fn a_file_that_is_not_there_is_an_error() {
        let missing = std::env::temp_dir().join("peekle-no-such-file-2f4a.txt");
        assert!(to_trash(&missing).is_err());
    }
}
