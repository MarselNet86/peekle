//! Screenshots on their way from the pasteboard into a session. tech.md 6.13.
//!
//! Control-Shift-Command-4 writes the image to the pasteboard and nowhere
//! else, while the only channel to an agent is a pty carrying text. So the
//! image has to become a file, and the file has to become a path in a reply.
//!
//! Everything that touches the real pasteboard sits behind [`Pasteboard`]. The
//! parts that decide what is a screenshot, where it lands and what goes on the
//! wire are free functions, so they stay testable with no AppKit in sight.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::types::ShotOffer;

/// The type a screenshot carries on the pasteboard.
pub const SCREENSHOT_TYPE: &str = "public.png";

/// Whether these pasteboard item types are a screenshot.
///
/// Captured, not assumed: `fixtures/pasteboard/screenshot.json` shows one item
/// carrying exactly `public.png`, while an image written by an app carries
/// `public.tiff` and copied text carries its own flavors alongside. The
/// declared types of the pasteboard itself are wider, because NSPasteboard
/// synthesises TIFF from a PNG, so the discriminator reads the item.
///
/// Types answer this and the contents never do: reading contents is what
/// raises the system paste prompt from macOS 15 on. tech.md 6.13.
pub fn is_screenshot(items: &[Vec<String>]) -> bool {
    match items {
        [only] => only.len() == 1 && only[0] == SCREENSHOT_TYPE,
        _ => false,
    }
}

/// `~/Library/Caches/peekle/shots`.
///
/// A cache and not the config directory: losing it costs nothing, and the path
/// has no space in it, which matters because it travels to the agent as a line
/// of a reply. tech.md 6.13.
pub fn shots_dir() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "peekle")?;
    Some(dirs.cache_dir().join("shots"))
}

/// Writes one screenshot and keeps the directory from growing forever.
pub fn write_shot(dir: &Path, id: &str, png: &[u8], keep: usize) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{id}.png"));
    std::fs::write(&path, png)?;
    prune(dir, keep);
    Ok(path)
}

/// Keeps the newest `keep` screenshots and deletes the rest.
///
/// Sorted by name rather than by mtime: the name is a ulid, so it already
/// sorts by the moment it was taken, and one readdir beats one stat per file.
/// A file that cannot be removed is left alone; a full cache is not worth an
/// error path in a feature that has nothing to do with it.
pub fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut shots: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    if shots.len() <= keep {
        return;
    }
    shots.sort();
    let doomed = shots.len() - keep;
    for path in shots.into_iter().take(doomed) {
        let _ = std::fs::remove_file(path);
    }
}

/// What one reply says once its attachments are counted in.
///
/// Every path takes a line of its own ahead of the text, because Claude Code
/// opens the file itself once it is named and needs nothing else. No `@` and
/// no quoting: `@` opens the file autocomplete in the TUI and turns a paste
/// into a mess, and the cache path has no space to escape. tech.md 6.13.
pub fn compose(text: &str, shots: &[String]) -> String {
    if shots.is_empty() {
        return text.to_string();
    }
    let mut lines: Vec<&str> = shots.iter().map(String::as_str).collect();
    if !text.is_empty() {
        lines.push(text);
    }
    lines.join("\n")
}

/// The pasteboard, behind a trait so tests never touch the real one.
///
/// Split in two on purpose. `change_count` and `item_types` are the silent
/// half and may be called on a timer; `read_png` is the half macOS may put a
/// paste prompt in front of, and it runs only once the user has agreed.
/// tech.md 6.13 and section 7.
pub trait Pasteboard: Send + Sync + 'static {
    /// Rises every time anything writes to the pasteboard.
    fn change_count(&self) -> i64;
    /// The type names of each item. Raises no prompt and copies no content.
    fn item_types(&self) -> Vec<Vec<String>>;
    /// The image bytes. Called once, after the user agreed to attach.
    fn read_png(&self) -> Option<Vec<u8>>;
}

/// The single offer that may stand at a time.
///
/// Resolved exactly once, by agreement, by expiry, or by the next screenshot
/// pushing it out. The discipline is the one blocking hooks get in section 8
/// and for a comparable reason: a leaked offer holds the Up arrow away from
/// every other application on the machine. tech.md 6.13.
#[derive(Debug, Default)]
pub struct OfferSlot {
    current: Mutex<Option<ShotOffer>>,
}

impl OfferSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts an offer up and hands back the one it replaced, if any.
    pub fn open(&self, offer: ShotOffer) -> Option<ShotOffer> {
        self.lock().replace(offer)
    }

    pub fn current(&self) -> Option<ShotOffer> {
        self.lock().clone()
    }

    /// Settles the offer. `None` means something else settled it first, and
    /// the caller must do nothing at all.
    pub fn take(&self) -> Option<ShotOffer> {
        self.lock().take()
    }

    /// Settles it only if its time is up.
    pub fn take_expired(&self, now: i64) -> Option<ShotOffer> {
        let mut current = self.lock();
        match current.as_ref() {
            Some(offer) if now >= offer.expires_at => current.take(),
            _ => None,
        }
    }

    /// A poisoned lock means an earlier holder panicked. Refusing to settle
    /// offers from then on would strand the Up arrow, which is worse.
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<ShotOffer>> {
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Scripted pasteboard. Records how often the contents were read, because
/// "the contents are read once, and only after the user agreed" is a rule of
/// 6.13 rather than an implementation detail. tech.md section 7.
#[derive(Debug, Default)]
pub struct FakePasteboard {
    inner: Mutex<FakeState>,
}

#[derive(Debug, Default)]
struct FakeState {
    change_count: i64,
    items: Vec<Vec<String>>,
    png: Option<Vec<u8>>,
    reads: u32,
}

impl FakePasteboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes to the fake pasteboard the way any application would.
    pub fn write(&self, items: Vec<Vec<String>>, png: Option<Vec<u8>>) {
        let mut state = self.lock();
        state.change_count += 1;
        state.items = items;
        state.png = png;
    }

    /// A screenshot, as `fixtures/pasteboard/screenshot.json` describes one.
    pub fn write_screenshot(&self, png: &[u8]) {
        self.write(vec![vec![SCREENSHOT_TYPE.to_string()]], Some(png.to_vec()));
    }

    /// How many times the contents were read. Every one of these is a system
    /// paste prompt the user might see.
    pub fn reads(&self) -> u32 {
        self.lock().reads
    }

    pub fn clear(&self) {
        let mut state = self.lock();
        state.change_count += 1;
        state.items = Vec::new();
        state.png = None;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Pasteboard for FakePasteboard {
    fn change_count(&self) -> i64 {
        self.lock().change_count
    }

    fn item_types(&self) -> Vec<Vec<String>> {
        self.lock().items.clone()
    }

    fn read_png(&self) -> Option<Vec<u8>> {
        let mut state = self.lock();
        state.reads += 1;
        state.png.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(id: &str, expires_at: i64) -> ShotOffer {
        ShotOffer {
            id: id.to_string(),
            session_id: "session".to_string(),
            project: "peekle".to_string(),
            created_at: 0,
            expires_at,
        }
    }

    #[test]
    fn a_screenshot_is_one_item_carrying_only_png() {
        assert!(is_screenshot(&[vec!["public.png".to_string()]]));
    }

    /// Everything else on a pasteboard has to stay silent, or the island
    /// offers to attach the address the user just copied.
    #[test]
    fn nothing_else_on_a_pasteboard_reads_as_a_screenshot() {
        for items in [
            vec![],
            vec![vec!["public.tiff".to_string()]],
            vec![vec!["public.utf8-plain-text".to_string()]],
            vec![vec!["public.file-url".to_string()]],
            vec![vec![
                "public.utf8-plain-text".to_string(),
                "public.html".to_string(),
            ]],
            vec![
                vec!["public.png".to_string()],
                vec!["public.png".to_string()],
            ],
            vec![vec![
                "public.png".to_string(),
                "public.file-url".to_string(),
            ]],
        ] {
            assert!(!is_screenshot(&items), "read as a screenshot: {items:?}");
        }
    }

    #[test]
    fn a_reply_with_no_attachment_is_the_text_itself() {
        assert_eq!(compose("look at this", &[]), "look at this");
    }

    #[test]
    fn every_attachment_takes_a_line_ahead_of_the_text() {
        let shots = vec!["/cache/a.png".to_string(), "/cache/b.png".to_string()];
        assert_eq!(
            compose("what is wrong here", &shots),
            "/cache/a.png\n/cache/b.png\nwhat is wrong here"
        );
    }

    /// The text is the user's and travels unchanged, attachment or not. Same
    /// rule the pty writes live by. tech.md 6.5.
    #[test]
    fn the_text_goes_through_untouched() {
        for text in [
            "say \"hi\"; echo $HOME `date`",
            "{ \"a\": [1, 2] }",
            "печатай юникод 🚀",
            "line one\nline two",
        ] {
            let composed = compose(text, &["/cache/a.png".to_string()]);
            assert_eq!(composed, format!("/cache/a.png\n{text}"));
        }
    }

    #[test]
    fn an_offer_is_settled_exactly_once() {
        let slot = OfferSlot::new();
        slot.open(offer("one", 100));

        assert!(slot.take().is_some(), "the first taker gets it");
        assert!(slot.take().is_none(), "and there is nothing left to take");
        assert!(slot.current().is_none());
    }

    /// The Up arrow belongs to one screenshot at a time, so the fresh one
    /// pushes the stale one out rather than queueing behind it.
    #[test]
    fn a_second_screenshot_replaces_the_first() {
        let slot = OfferSlot::new();
        slot.open(offer("one", 100));

        let replaced = slot.open(offer("two", 200));
        assert_eq!(replaced.map(|o| o.id), Some("one".to_string()));
        assert_eq!(slot.current().map(|o| o.id), Some("two".to_string()));
    }

    #[test]
    fn expiry_only_settles_an_offer_whose_time_is_up() {
        let slot = OfferSlot::new();
        slot.open(offer("one", 100));

        assert!(slot.take_expired(99).is_none(), "still standing");
        assert!(slot.take_expired(100).is_some(), "the deadline is the edge");
        assert!(slot.take_expired(1000).is_none(), "already settled");
    }

    /// Agreement arriving after expiry must find nothing: the file write, the
    /// event and the view change all hang off the take.
    #[test]
    fn agreement_after_expiry_finds_nothing_left() {
        let slot = OfferSlot::new();
        slot.open(offer("one", 100));

        assert!(slot.take_expired(100).is_some());
        assert!(slot.take().is_none());
    }

    #[test]
    fn the_fake_pasteboard_reports_a_screenshot_and_reads_nothing_by_itself() {
        let pasteboard = FakePasteboard::new();
        pasteboard.write_screenshot(b"png bytes");

        assert_eq!(pasteboard.change_count(), 1);
        assert!(is_screenshot(&pasteboard.item_types()));
        assert_eq!(pasteboard.reads(), 0, "detection reads no contents");

        assert_eq!(pasteboard.read_png(), Some(b"png bytes".to_vec()));
        assert_eq!(pasteboard.reads(), 1);
    }

    #[test]
    fn writing_a_shot_prunes_the_cache_to_the_cap() {
        let dir = std::env::temp_dir().join(format!("peekle-shots-{}", ulid::Ulid::generate()));
        for index in 0..5 {
            write_shot(&dir, &format!("0000{index}"), b"png", 3).expect("write");
        }

        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        left.sort();
        assert_eq!(left, vec!["00002.png", "00003.png", "00004.png"]);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    /// The cache lives under a directory the user can delete at any moment,
    /// and a screenshot that cannot be written is a message, not a panic.
    #[test]
    fn a_directory_that_cannot_be_created_is_an_error_and_not_a_panic() {
        let path = Path::new("/dev/null/nowhere");
        assert!(write_shot(path, "id", b"png", 3).is_err());
        prune(path, 3);
    }
}
