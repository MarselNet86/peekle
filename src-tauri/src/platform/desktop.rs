//! Windows and Linux, through the cross-platform API of Tauri. tech.md 6.27.
//!
//! No NSPanel here, and no substitute pretending to be one. What this file
//! promises is exactly what 6.27 lists: a window over everything on the
//! primary display that takes clicks by view, takes focus only when it has a
//! field, watches the clipboard for a picture, and puts a transcript in the
//! system's own trash. What it does not promise is said at the function that
//! cannot promise it.

use std::path::Path;
use std::sync::Mutex;

use peekle_core::shots::Pasteboard;
use peekle_core::types::IslandView;
use tauri::AppHandle;

use super::{focusable_for, image_signature, position, window, Notch, PanelError, ISLAND};

/// Sets the window up once at startup. It is shown and hidden afterwards,
/// never created and destroyed. tech.md 6.7 and 6.27.
pub fn convert_all(app: &AppHandle) -> Result<(), PanelError> {
    let window = window(app, ISLAND)?;
    // Over everything, out of the taskbar, and asleep to the mouse until a
    // view says otherwise. `set_view` switches the last two per view.
    window.set_always_on_top(true)?;
    window.set_skip_taskbar(true)?;
    window.set_focusable(false)?;
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

/// There is no notch to hug on these platforms, so the island rests as the
/// floating pill 6.7 draws on any display without one. tech.md 6.27.
pub fn notch_for(_size: (f64, f64)) -> Option<Notch> {
    None
}

/// The primary display, as logical origin and size, and only ever that one.
///
/// On macOS the island follows the pointer because the notch is a property of
/// the display. Here there is no notch, displays differ in size and scale, and
/// an island that jumps between them after the hand reads as a window that
/// does not know where it lives. Screenshots still come from any display: the
/// system's own tool takes them, and the clipboard is one. tech.md 6.27.
pub(super) fn active_screen(app: &AppHandle) -> Option<((f64, f64), (f64, f64))> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let origin = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    Some(((origin.x, origin.y), (size.width, size.height)))
}

/// No global pointer monitor here. The tracker of 6.7 keeps its 100ms poll as
/// the fallback for exactly this, at the cost 6.27 names: a click that lands
/// on the mark inside one tick goes through to what is under it. tech.md 6.27.
pub fn watch_pointer<F>(_app: &AppHandle, _moved: F) -> Result<(), String>
where
    F: Fn(&AppHandle) + 'static,
{
    Err("no global pointer monitor on this platform, polling instead".to_string())
}

/// Places the window and shows it. Re-asserted on every open, as on macOS:
/// the top-most flag is the one promise the product cannot degrade on.
pub fn show(app: &AppHandle, label: &str) -> Result<(), PanelError> {
    position(app, label)?;
    let window = window(app, label)?;
    window.show()?;
    window.set_always_on_top(true)?;
    tracing::debug!(label, "window shown");
    Ok(())
}

/// A file dialog on these platforms comes up over the window that asked for
/// it, and the island is a window; nothing has to be activated first.
pub fn take_front() {}

/// And nothing has to be given back.
pub fn give_front_back() {}

/// The view changed, and with it whether the window may take focus: the
/// substitute for `becomes_key_only_if_needed` that 6.27 spells out.
pub fn apply_view(app: &AppHandle, view: &IslandView) {
    let focusable = focusable_for(view);
    match window(app, ISLAND) {
        Ok(window) => {
            if let Err(err) = window.set_focusable(focusable) {
                tracing::error!(error = %err, focusable, "failed to switch focusability");
            }
        }
        Err(err) => tracing::error!(error = %err, "no island window to switch focus on"),
    }
}

/// The system's own trash: the Recycle Bin, or the freedesktop trash. The
/// same promise as 6.26 -- the file can be taken back from where every other
/// deletion on the machine goes.
pub fn to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|err| err.to_string())
}

/// Hands a URL to the browser the person chose. tech.md 6.16 and 6.22.
///
/// Windows through `rundll32 url.dll` rather than `cmd /c start`: `start`
/// reads `&` in a query string as a command separator, and an OAuth URL is
/// nothing but query string. Linux through `xdg-open`, which every desktop
/// that ships a browser ships too.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    #[cfg(not(windows))]
    let mut command = std::process::Command::new("xdg-open");

    command
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

/// The type name 6.13 recognises a screenshot by. The clipboards here have
/// no such vocabulary, so the one thing they can say -- a picture is on the
/// clipboard -- is said in the words the shared rule already understands.
const PICTURE: &str = "public.png";

/// The clipboard, watched. tech.md 6.13 and 6.27.
///
/// Reading the picture is what turns it into a file, on every platform. What
/// differs is how a write is noticed: Windows keeps a change counter of its
/// own, Linux does not, and the poll there reads the picture and signs it.
pub struct SystemPasteboard {
    /// The last signature seen and the counter it stands behind, for the
    /// platform that has no counter of its own.
    signed: Mutex<(u64, i64)>,
}

impl Default for SystemPasteboard {
    fn default() -> Self {
        Self {
            signed: Mutex::new((0, 0)),
        }
    }
}

impl SystemPasteboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// The picture on the clipboard as raw RGBA, or None when there is none.
    fn picture() -> Option<arboard::ImageData<'static>> {
        let mut clipboard = arboard::Clipboard::new().ok()?;
        clipboard.get_image().ok().map(|image| image.to_owned_img())
    }

    /// The picture as PNG bytes, which is what the shots cache holds.
    fn encode(image: &arboard::ImageData<'_>) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, image.width as u32, image.height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&image.bytes).ok()?;
        }
        Some(out)
    }
}

impl Pasteboard for SystemPasteboard {
    #[cfg(target_os = "windows")]
    fn change_count(&self) -> i64 {
        use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;

        // The system's own counter, which rises on every write and reads
        // nothing. tech.md 6.27.
        // SAFETY: a plain query with no arguments and no side effects.
        i64::from(unsafe { GetClipboardSequenceNumber() })
    }

    #[cfg(not(target_os = "windows"))]
    fn change_count(&self) -> i64 {
        // No counter here, so the picture is read and signed on every tick and
        // the counter rises when the signature moves. Reading is allowed: no
        // system asks for permission to paste on this platform, which is the
        // one reason 6.13 reads only types. tech.md 6.27 and R-23.
        let signature = Self::picture()
            .map(|image| image_signature(image.width, image.height, &image.bytes))
            .unwrap_or(0);
        let mut signed = self
            .signed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if signed.0 != signature {
            signed.0 = signature;
            signed.1 += 1;
        }
        signed.1
    }

    #[cfg(target_os = "windows")]
    fn item_types(&self) -> Vec<Vec<String>> {
        use windows::core::w;
        use windows::Win32::System::DataExchange::{
            IsClipboardFormatAvailable, RegisterClipboardFormatW,
        };
        use windows::Win32::System::Ole::CF_DIB;

        // A picture is on the clipboard when either the PNG format the
        // snipping tool writes or the device-independent bitmap every image
        // carries is available. Neither read copies anything.
        // SAFETY: format queries with no side effects; the format name is a
        // static wide string.
        let present = unsafe {
            let png = RegisterClipboardFormatW(w!("PNG"));
            (png != 0 && IsClipboardFormatAvailable(png).is_ok())
                || IsClipboardFormatAvailable(u32::from(CF_DIB.0)).is_ok()
        };
        if present {
            vec![vec![PICTURE.to_string()]]
        } else {
            Vec::new()
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn item_types(&self) -> Vec<Vec<String>> {
        // The read that `change_count` already did is what says whether a
        // picture is there; it is read again rather than cached, because the
        // clipboard may have moved between the two calls.
        if Self::picture().is_some() {
            vec![vec![PICTURE.to_string()]]
        } else {
            Vec::new()
        }
    }

    fn read_png(&self) -> Option<Vec<u8>> {
        Self::encode(&Self::picture()?)
    }
}
