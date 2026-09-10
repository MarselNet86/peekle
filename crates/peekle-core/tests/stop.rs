//! S22 acceptance: what `Stop` puts on each channel, and what comes back.
//! tech.md 6.5.

use peekle_core::inbox::STOP_REQUEST;
use peekle_core::pty::{message_writes, INTERRUPT};
use peekle_core::sessions::{FeedEvent, SessionRegistry};
use peekle_core::transcripts::{card_from_lines, is_stop_request, spoken, ASKED_TO_STOP};
use peekle_core::types::{EntryKind, EntryState, SessionRef};

/// Esc, alone. Not a line: a newline after it would be a second key, and an
/// empty submit at that.
#[test]
fn the_interrupt_is_one_escape_byte_and_no_newline() {
    assert_eq!(INTERRUPT, b"\x1b");
    assert!(!INTERRUPT.contains(&b'\r'));
    assert!(!INTERRUPT.contains(&b'\n'));
    // And it is not what a message write looks like.
    assert_ne!(message_writes("\x1b").concat(), INTERRUPT);
}

/// The wrapper the receiver writes around a message delivered to its inbox,
/// copied from a live 2.1.263 record of this very request (2026-09-10).
fn as_it_comes_back(text: &str) -> String {
    format!(
        "Another Claude session sent a message:\n{text}\n\nThis came from another Claude session — not typed by your user, but very likely working on their behalf."
    )
}

/// The request goes out as a `user` message, because the inbox has no
/// interrupt frame -- and comes back through the same hook as anything a
/// person types. It is still not a turn: the island wrote those words, and
/// 6.11 forbids a `user` record nobody typed from becoming a bubble, a title
/// or a confirmation. tech.md 6.5.
#[test]
fn the_stop_request_is_never_read_as_something_a_person_said() {
    assert!(!STOP_REQUEST.trim().is_empty());

    // Bare, which is how the live hook carries it.
    assert!(is_stop_request(STOP_REQUEST));
    assert_eq!(spoken(STOP_REQUEST), None);

    // Wrapped in the prose the receiver writes, which is how the file does.
    let wrapped = as_it_comes_back(STOP_REQUEST);
    assert!(is_stop_request(&wrapped));
    assert_eq!(spoken(&wrapped), None);

    // And in the tag older transcripts carry.
    let tagged = format!(
        "Another Claude session sent a message:\n<cross-session-message from=\"uds:/tmp/cc-socks/1.sock\">\n{STOP_REQUEST}\n</cross-session-message>\n\nThis came from another Claude session."
    );
    assert!(is_stop_request(&tagged));
    assert_eq!(spoken(&tagged), None);
}

/// Only this text, and only whole. A person quoting the island, or a peer
/// message somebody actually sent, is words like any other.
#[test]
fn everything_else_delivered_the_same_way_is_still_words() {
    let peer = as_it_comes_back("have a look at the panel code");
    assert!(!is_stop_request(&peer));
    assert_eq!(
        spoken(&peer).as_deref(),
        Some("have a look at the panel code")
    );

    let quoted = format!("I pressed the button and it sent: {STOP_REQUEST}");
    assert!(!is_stop_request(&quoted));
    assert_eq!(spoken(&quoted).as_deref(), Some(quoted.as_str()));
}

/// The live hook path. No event at all, so none of the three consequences
/// 6.11 names can follow -- and the one that bites hardest is the third: a
/// reply of the person's still in flight to another chat used to be marked
/// delivered by this. tech.md 6.5 and 6.11.
#[test]
fn the_hook_makes_no_turn_and_confirms_nothing() {
    let payload = serde_json::json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": "s1",
        "cwd": "/tmp/peekle",
        "prompt": as_it_comes_back(STOP_REQUEST),
    });

    assert!(FeedEvent::from_payload(&payload).is_none());

    // And the reply that was in flight is still in flight.
    let session = SessionRef {
        session_id: "s1".to_string(),
        cwd: "/tmp/peekle".to_string(),
        project: "peekle".to_string(),
        pid: None,
        tty: None,
    };
    let mut registry = SessionRegistry::new();
    registry.claim(&session.session_id);
    registry.open_owned(session.clone(), 1);
    registry.user_turn(
        session.clone(),
        "the one I actually sent",
        EntryState::Running,
        2,
    );

    assert!(!registry.confirm_spoken(&session.session_id, STOP_REQUEST, 3));
    let card = &registry.cards()[0];
    assert_eq!(card.entries[0].state, EntryState::Running);
}

/// The file path. One line about the conversation, in nobody's colour, the
/// way `Compacted` and `Switched to …` already stand -- and never the title.
#[test]
fn the_file_shows_it_as_a_line_about_the_conversation() {
    let asked = as_it_comes_back(STOP_REQUEST).replace('\n', "\\n");
    let lines = [
        format!(
            r#"{{"type":"user","uuid":"u1","sessionId":"s","cwd":"/tmp/p","timestamp":"2026-09-10T11:40:00.000Z","message":{{"role":"user","content":"{asked}"}}}}"#
        ),
        r#"{"type":"assistant","uuid":"a1","sessionId":"s","timestamp":"2026-09-10T11:40:20.000Z","message":{"role":"assistant","model":"claude-opus-5","usage":{"input_tokens":1},"content":[{"type":"text","text":"Stopped: I was halfway through the fixtures."}]}}"#.to_string(),
    ];

    let card = card_from_lines(lines.iter(), "s", 0).expect("a dialogue");

    assert_eq!(card.entries[0].kind, EntryKind::Notice);
    assert_eq!(card.entries[0].text, ASKED_TO_STOP);
    assert_eq!(card.entries[1].kind, EntryKind::Assistant);
    assert!(
        !card.entries.iter().any(|e| e.kind == EntryKind::User),
        "nobody said it"
    );
    assert!(
        !card.title.contains("End this turn"),
        "and it never names the chat: {}",
        card.title
    );
}

/// The request stands on the card until the turn ends, so the button takes
/// one press and waits. A second press is a second message and a second turn
/// in somebody's chat. tech.md 6.5.
#[test]
fn a_standing_request_is_carried_by_the_card_until_the_turn_ends() {
    let session = SessionRef {
        session_id: "s1".to_string(),
        cwd: "/tmp/peekle".to_string(),
        project: "peekle".to_string(),
        pid: None,
        tty: None,
    };
    let mut registry = SessionRegistry::new();
    registry.ensure(session.clone(), 1);

    assert!(registry.start_stop(&session.session_id, 1_000));
    assert_eq!(registry.cards()[0].stopping, Some(1_000));
    assert!(!registry.start_stop("nobody", 1_000), "no card, no claim");

    assert!(registry.end_stop(&session.session_id));
    assert_eq!(registry.cards()[0].stopping, None);
    assert!(!registry.end_stop(&session.session_id));
}

/// And a request nothing ever answered gives the button back by itself: a
/// chat that ignores it must not take the control away for good. tech.md 6.5.
#[test]
fn a_request_nobody_answered_is_given_up_on() {
    use peekle_core::sessions::STOP_WAIT_MS;

    let session = SessionRef {
        session_id: "s1".to_string(),
        cwd: "/tmp/peekle".to_string(),
        project: "peekle".to_string(),
        pid: None,
        tty: None,
    };
    let mut registry = SessionRegistry::new();
    registry.ensure(session.clone(), 1);
    registry.start_stop(&session.session_id, 1_000);

    assert!(!registry.rest_stale_stops(1_000 + STOP_WAIT_MS - 1, STOP_WAIT_MS));
    assert!(registry.cards()[0].stopping.is_some());

    assert!(registry.rest_stale_stops(1_000 + STOP_WAIT_MS, STOP_WAIT_MS));
    assert_eq!(registry.cards()[0].stopping, None);
}
