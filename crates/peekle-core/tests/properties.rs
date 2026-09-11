//! Property based tests for the pure logic of the core crate.
//! tech.md section 10 names these: usage window math and the label classifier.

use peekle_core::auth::{authorize_url, logged_in, strip_escapes, wants_code};
use peekle_core::island::{shape_rect, Rect};
use peekle_core::labels::classify;
use peekle_core::shots::compose;
use peekle_core::types::{clamp_pct, UsageWindow, UsageWindowStat};
use proptest::prelude::*;

proptest! {
    /// The classifier feeds the HUD directly. A panic there takes down a
    /// window the user cannot reopen, so it has to be total over any title.
    #[test]
    fn classify_is_total(title in ".*") {
        let _ = classify(&title);
    }

    #[test]
    fn classify_is_stable(title in ".*") {
        prop_assert_eq!(classify(&title), classify(&title));
    }

    /// Case folding must not change the verdict, otherwise a title typed in
    /// caps lands in a different lane than the same title in lower case.
    #[test]
    fn classify_ignores_case(title in "[a-zA-Z ]{0,64}") {
        prop_assert_eq!(classify(&title), classify(&title.to_uppercase()));
    }

    /// Rate limit headers are undocumented. Whatever number arrives, the bar
    /// has to render.
    #[test]
    fn clamp_pct_lands_in_range(pct in proptest::num::f32::ANY) {
        let clamped = clamp_pct(pct);
        prop_assert!((0.0..=100.0).contains(&clamped));
        prop_assert!(!clamped.is_nan());
    }

    #[test]
    fn clamp_pct_is_idempotent(pct in proptest::num::f32::ANY) {
        prop_assert_eq!(clamp_pct(clamp_pct(pct)), clamp_pct(pct));
    }

    #[test]
    fn clamp_pct_is_monotone(a in -1.0e6f32..1.0e6, b in -1.0e6f32..1.0e6) {
        prop_assume!(a <= b);
        prop_assert!(clamp_pct(a) <= clamp_pct(b));
    }

    /// A utilization fraction of 0..1 scaled by 100 always yields a usable bar.
    #[test]
    fn window_stat_accepts_any_utilization(fraction in proptest::num::f32::ANY) {
        let stat = UsageWindowStat::new(UsageWindow::FiveHour, fraction * 100.0, None);
        prop_assert!((0.0..=100.0).contains(&stat.used_pct));
    }
}

proptest! {
    /// The resting mark is the only part of a collapsed island that takes a
    /// click. A rectangle outside the window would take clicks where nothing
    /// is drawn, which is the failure the user notices and cannot explain.
    #[test]
    fn the_rest_mark_never_leaves_the_window(
        w in 1.0f64..4000.0,
        h in 1.0f64..4000.0,
        x in -4000.0f64..4000.0,
        y in -4000.0f64..4000.0,
        mark_x in proptest::num::f64::ANY,
        mark_y in proptest::num::f64::ANY,
        mark_w in proptest::num::f64::ANY,
        mark_h in proptest::num::f64::ANY,
    ) {
        let window = Rect::new(x, y, w, h);
        let mark = Rect::new(mark_x, mark_y, mark_w, mark_h);
        let Some(rect) = shape_rect(window, mark) else { return Ok(()) };

        prop_assert!(rect.x >= window.x);
        prop_assert!(rect.y >= window.y);
        prop_assert!(rect.x + rect.width <= window.x + window.width + f64::EPSILON);
        prop_assert!(rect.y + rect.height <= window.y + window.height + f64::EPSILON);
    }

    /// Every point the hotspot claims has to be a point the window covers,
    /// otherwise Rust hands the mouse to a window that draws nothing there.
    #[test]
    fn every_point_of_the_mark_is_a_point_of_the_window(
        mark_x in 0.0f64..800.0,
        mark_y in 0.0f64..600.0,
        mark_w in 1.0f64..2000.0,
        mark_h in 1.0f64..2000.0,
        px in -100.0f64..900.0,
        py in -100.0f64..800.0,
    ) {
        let window = Rect::new(0.0, 0.0, 720.0, 560.0);
        let mark = Rect::new(mark_x, mark_y, mark_w, mark_h);
        let Some(rect) = shape_rect(window, mark) else { return Ok(()) };

        if rect.contains((px, py)) {
            prop_assert!(window.contains((px, py)));
        }
    }

    /// The webview says where the shape is, and the hotspot is there and
    /// nowhere else: a shape off the top edge (tech.md 6.7) must not take a
    /// click at the edge. tech.md 6.5.
    #[test]
    fn the_hotspot_is_where_the_shape_was_drawn(
        mark_x in 0.0f64..600.0,
        mark_y in 0.0f64..400.0,
        mark_w in 1.0f64..100.0,
        mark_h in 1.0f64..100.0,
    ) {
        let window = Rect::new(50.0, 20.0, 720.0, 560.0);
        let mark = Rect::new(mark_x, mark_y, mark_w, mark_h);
        let rect = shape_rect(window, mark).expect("a mark inside the window is a hotspot");

        prop_assert_eq!(rect.x, window.x + mark_x);
        prop_assert_eq!(rect.y, window.y + mark_y);
        prop_assert!(rect.contains((window.x + mark_x, window.y + mark_y)));
        prop_assert!(!rect.contains((window.x + mark_x, window.y + mark_y - 0.5)));
    }
}

proptest! {
    /// A reply with attachments still carries the user's words exactly. The
    /// paths are added, nothing is escaped, and nothing is rewritten: the same
    /// rule the pty write lives by. tech.md 6.13 and 6.5.
    #[test]
    fn the_text_survives_any_number_of_attachments(
        text in "[^\n]{1,80}",
        shots in proptest::collection::vec("/tmp/[a-z0-9]{1,20}\\.png", 0..4),
    ) {
        let composed = compose(&text, &shots);
        prop_assert!(composed.ends_with(&text));

        let lines: Vec<&str> = composed.split('\n').collect();
        prop_assert_eq!(lines.len(), shots.len() + 1);
        for (line, shot) in lines.iter().zip(shots.iter()) {
            prop_assert_eq!(line, shot);
        }
    }

    /// Order is the contract: every path stands on its own line ahead of the
    /// text, whichever paths they are.
    #[test]
    fn attachments_keep_their_order(
        shots in proptest::collection::vec("/tmp/[a-z0-9]{1,20}\\.png", 1..5),
    ) {
        let composed = compose("look", &shots);
        let mut at = 0usize;
        for shot in &shots {
            let found = composed[at..].find(shot.as_str());
            prop_assert!(found.is_some());
            at += found.unwrap_or(0) + shot.len();
        }
    }
}

/// The byte a terminal starts an escape sequence with.
const ESC: char = '\u{1b}';

proptest! {
    /// The sign-in reads whatever `claude auth login` puts on the wire, and a
    /// pty carries arbitrary bytes: half an escape sequence at the edge of a
    /// read, a multi-byte character split in two, a terminal drawing things
    /// nobody planned for. A panic on the reader thread would leave the panel
    /// waiting on a run that has already died. tech.md 6.16.
    #[test]
    fn reading_the_sign_in_output_is_total(chunk in ".*") {
        let _ = strip_escapes(&chunk);
        let _ = authorize_url(&chunk);
        let _ = wants_code(&chunk);
    }

    /// Whatever it hands back is an address, not a fragment of one: the panel
    /// offers it as a link, and half an address is a dead end dressed as a way
    /// out.
    #[test]
    fn any_address_it_finds_is_whole(chunk in ".*") {
        if let Some(url) = authorize_url(&chunk) {
            prop_assert!(url.starts_with("https://"));
            prop_assert!(url.len() > "https://".len());
            prop_assert!(!url.chars().any(char::is_whitespace));
            prop_assert!(!url.contains(ESC));
        }
    }

    /// `auth status` is a subprocess whose output nothing here controls: an
    /// older CLI, a version that renamed the field, an error on stdout. None
    /// of it may panic, and none of it may come back as `Some(false)` -- that
    /// would send a working account through a login it does not need.
    /// tech.md 6.16.
    #[test]
    fn reading_the_status_never_invents_a_no(raw in ".*") {
        if logged_in(&raw) == Some(false) {
            prop_assert!(raw.contains("loggedIn"));
        }
    }
}
