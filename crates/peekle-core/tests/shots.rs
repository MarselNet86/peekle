#![allow(clippy::unwrap_used)]
//! S15 against the captured pasteboard. Every case here is what macOS itself
//! wrote through `scripts/capture-pasteboard.sh`, because the screenshot
//! discriminator is a claim about someone else's format and the product acts
//! on it. tech.md 6.13 and rule 6.

use peekle_core::shots::{
    compose, dismissed_by_key, is_screenshot, FakeKeys, FakePasteboard, Keys, OfferSlot,
    Pasteboard, KEY_GRACE_MS,
};
use peekle_core::types::ShotOffer;

fn items(case: &str) -> Vec<Vec<String>> {
    let raw = match case {
        "screenshot" => include_str!("../../../tests/fixtures/pasteboard/screenshot.json"),
        "image" => include_str!("../../../tests/fixtures/pasteboard/image.json"),
        "text" => include_str!("../../../tests/fixtures/pasteboard/text.json"),
        "text-from-browser" => {
            include_str!("../../../tests/fixtures/pasteboard/text-from-browser.json")
        }
        "file" => include_str!("../../../tests/fixtures/pasteboard/file.json"),
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
    let raw = include_str!("../../../tests/fixtures/pasteboard/screenshot.json");
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

/// v86, the fourth way an offer settles. The person took a screenshot they did
/// not mean to send and kept typing: the pill goes, and it goes on the key
/// rather than on the rest of its five seconds.
#[test]
fn any_other_key_settles_the_offer() {
    let keys = FakeKeys::new();
    let at_open = keys.counted();

    keys.press(0);
    keys.age(KEY_GRACE_MS);

    assert!(dismissed_by_key(
        at_open,
        keys.counted(),
        keys.since_keystroke_ms()
    ));
}

/// Nobody typed, so the offer stands its full life and settles on its own
/// clock. A pill that vanished on nothing would be worse than one that waits.
#[test]
fn an_untouched_keyboard_leaves_the_offer_alone() {
    let keys = FakeKeys::new();
    let at_open = keys.counted();

    for _ in 0..100 {
        keys.age(50);
        assert!(!dismissed_by_key(
            at_open,
            keys.counted(),
            keys.since_keystroke_ms()
        ));
    }
}

/// The keystroke that took the screenshot is older than the offer it produced:
/// ⌃⇧⌘4 is pressed before the image reaches the pasteboard, and the watch only
/// sees the write afterwards. Counting it would settle every offer at birth.
#[test]
fn the_keystroke_that_took_the_screenshot_does_not_settle_it() {
    let keys = FakeKeys::new();
    keys.press(0);

    // The capture, the drag, and a tick of the watch, all before the offer.
    keys.age(1_200);
    let at_open = keys.counted();
    keys.age(KEY_GRACE_MS);

    assert!(!dismissed_by_key(
        at_open,
        keys.counted(),
        keys.since_keystroke_ms()
    ));
}

/// The Up arrow is a keystroke like any other and lands at the HID level
/// before Carbon calls the handler. Inside the grace the fast tick keeps its
/// hands off, or agreement would arrive to find nothing left to attach.
#[test]
fn the_attach_key_is_answered_before_it_can_dismiss_anything() {
    let keys = FakeKeys::new();
    let at_open = keys.counted();

    keys.press(0);
    for ago in [0, 1, 100, KEY_GRACE_MS - 1] {
        assert!(
            !dismissed_by_key(at_open, keys.counted(), ago),
            "settled {ago}ms in, ahead of the shortcut handler"
        );
    }

    let slot = OfferSlot::new();
    slot.open(ShotOffer {
        id: "01J".to_string(),
        session_id: "session".to_string(),
        project: "peekle".to_string(),
        created_at: 0,
        expires_at: 5_000,
    });

    // The handler gets there first, and the tick that follows finds the offer
    // already settled: one resolution, never two. tech.md 6.13.
    assert!(slot.take().is_some(), "agreement takes the offer");
    assert!(
        slot.take().is_none(),
        "and the late tick has nothing to take"
    );
}

/// The counter belongs to the system and wraps where u32 does. A comparison
/// that asked whether it grew would go blind for one keystroke in four
/// billion, which is a bug nobody could ever reproduce.
#[test]
fn the_keystroke_counter_may_wrap() {
    assert!(dismissed_by_key(u32::MAX, 0, KEY_GRACE_MS));
    assert!(!dismissed_by_key(u32::MAX, u32::MAX, KEY_GRACE_MS));
}

/// The offer may know how many keys were pressed and how long ago, and nothing
/// else. There is no way to ask this trait which key it was, and that is what
/// keeps Accessibility out of the product. tech.md 6.13 and 6.9.
#[test]
fn the_keyboard_is_only_ever_a_count_and_an_age() {
    let keys = FakeKeys::new();
    keys.press(10);

    let counted: u32 = keys.counted();
    let age: i64 = keys.since_keystroke_ms();
    assert_eq!(counted, 1);
    assert_eq!(age, 10);
}
