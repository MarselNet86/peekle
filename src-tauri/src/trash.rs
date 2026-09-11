//! Putting a file in the macOS Trash. tech.md 6.26.
//!
//! `NSFileManager.trashItemAtURL` and never `std::fs::remove_file`: a
//! transcript is a person's own conversation, and "this chat is gone" is a
//! decision they must be able to take back the way they take back every other
//! deletion on this machine -- in Finder. It is also what Finder itself calls,
//! so the file lands where the user looks for it.

use std::path::Path;

use objc2_foundation::{NSFileManager, NSString, NSURL};

/// Moves one file to the Trash.
///
/// A refusal from the system -- no permission, a volume with no Trash -- is an
/// error and never a silent success: the row is about to disappear, and it may
/// only disappear if the file actually went somewhere. tech.md 6.26.
pub fn to_trash(path: &Path) -> Result<(), String> {
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
