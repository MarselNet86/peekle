#![allow(clippy::unwrap_used)]
//! S15 against the captured pasteboard. Every case here is what macOS itself
//! wrote through `scripts/capture-pasteboard.sh`, because the screenshot
//! discriminator is a claim about someone else's format and the product acts
//! on it. tech.md 6.13 and rule 6.

use peekle_core::shots::{compose, is_screenshot, FakePasteboard, OfferSlot, Pasteboard};
use peekle_core::types::ShotOffer;

fn items(case: &str) -> Vec<Vec<String>> {
    let raw = match case {
        "screenshot" => include_str!("../../../fixtures/pasteboard/screenshot.json"),
        "image" => include_str!("../../../fixtures/pasteboard/image.json"),
        "text" => include_str!("../../../fixtures/pasteboard/text.json"),
        "text-from-browser" => include_str!("../../../fixtures/pasteboard/text-from-browser.json"),
        "file" => include_str!("../../../fixtures/pasteboard/file.json"),
        other => panic!("no fixture captured for {other}"),
    };
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    serde_json::from_value(value["items"].clone()).unwrap()
}

#[test]
fn the_captured_screenshot_is_recognised() {
    assert!(is_screenshot(&items("screenshot")));
}

/// The offer costs the user the Up arrow and a line across their screen, so
/// every other thing a pasteboard holds has to leave it alone.
#[test]
fn nothing_else_that_was_captured_is_recognised() {
    for case in ["image", "text", "text-from-browser", "file"] {
        assert!(!is_screenshot(&items(case)), "{case} read as a screenshot");
    }
}

/// The pasteboard declares more types than its item carries: NSPasteboard
/// synthesises TIFF from a PNG. Reading the declared list instead of the item
/// would make every image copy look like a screenshot.
#[test]
fn the_declared_types_are_wider_than_the_item() {
    let raw = include_str!("../../../fixtures/pasteboard/screenshot.json");
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let declared: Vec<String> = serde_json::from_value(value["types"].clone()).unwrap();

    assert!(declared.contains(&"public.tiff".to_string()));
    assert_eq!(items("screenshot"), vec![vec!["public.png".to_string()]]);
}

/// Detection runs on a timer, and from macOS 15 a read of the contents is a
/// system paste prompt. A tick that reads bytes would ask the user for
/// permission every time anyone copied anything. tech.md 6.13 and R-13.
#[test]
fn detecting_a_screenshot_never_reads_the_contents() {
    let pasteboard = FakePasteboard::new();
    pasteboard.write_screenshot(b"png");

    for _ in 0..10 {
        let _ = pasteboard.change_count();
        assert!(is_screenshot(&pasteboard.item_types()));
    }
    assert_eq!(pasteboard.reads(), 0);
}

/// The error path of 6.13: the user copies something else between the offer
/// and their answer, so there is no screenshot left to attach.
#[test]
fn a_pasteboard_emptied_before_the_answer_hands_back_nothing() {
    let pasteboard = FakePasteboard::new();
    pasteboard.write_screenshot(b"png");

    let slot = OfferSlot::new();
    slot.open(ShotOffer {
        id: "01J".to_string(),
        session_id: "session".to_string(),
        project: "peekle".to_string(),
        created_at: 0,
        expires_at: 5_000,
    });

    pasteboard.clear();
    assert!(slot.take().is_some(), "the offer is still answerable");
    assert_eq!(pasteboard.read_png(), None, "but there is nothing to write");
}

/// What the agent receives. The path names the file and Claude Code opens it
/// itself; nothing else is added, and the user's own text is untouched.
#[test]
fn an_attached_reply_names_the_file_and_then_speaks() {
    let composed = compose(
        "what is wrong with this layout",
        &["/Users/x/Library/Caches/peekle/shots/01J.png".to_string()],
    );
    assert_eq!(
        composed,
        "/Users/x/Library/Caches/peekle/shots/01J.png\nwhat is wrong with this layout"
    );
    assert!(!composed.contains('@'), "no file autocomplete in the TUI");
}
