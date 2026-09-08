//! S22 acceptance: what `Stop` puts on each channel. tech.md 6.5.

use peekle_core::inbox::STOP_REQUEST;
use peekle_core::pty::{message_writes, INTERRUPT};
use peekle_core::transcripts::spoken;

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

/// The request a foreign process gets is a plain turn: it reaches the feed
/// through the same unwrapping as anything else typed, as itself.
#[test]
fn the_stop_request_is_a_turn_that_reads_as_itself() {
    assert!(!STOP_REQUEST.trim().is_empty());
    assert!(STOP_REQUEST.starts_with("Stop"));
    assert_eq!(spoken(STOP_REQUEST).as_deref(), Some(STOP_REQUEST));
    let wrapped = format!(
        "Another Claude session sent a message:\n<cross-session-message from=\"uds:/tmp/cc-socks/1.sock\">\n{STOP_REQUEST}\n</cross-session-message>\n\nThis came from another Claude session."
    );
    assert_eq!(spoken(&wrapped).as_deref(), Some(STOP_REQUEST));
}
