#![allow(clippy::unwrap_used)]
//! S21 acceptance: where a reply to an observed chat goes is a function of
//! the registry Claude Code keeps, and it never lets two agents onto one
//! transcript. tech.md 6.5 and R-17.

use std::path::{Path, PathBuf};

use peekle_core::registry::{find, parse_record, route, LiveSession, Route, PEER_PROTOCOL};
use proptest::prelude::*;

fn live(pid: u32, session_id: &str, inbox: Option<&str>, protocol: Option<u64>) -> LiveSession {
    LiveSession {
        pid,
        session_id: session_id.to_string(),
        cwd: "/tmp/proj".to_string(),
        proc_start: Some("Tue Sep  8 08:32:47 2026".to_string()),
        version: "2.1.261".to_string(),
        entrypoint: Some("claude-desktop".to_string()),
        peer_protocol: protocol,
        inbox: inbox.map(PathBuf::from),
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("peekle-{name}-{}", ulid::Ulid::generate()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Nobody holds the chat: resume it now, not in ninety seconds.
#[test]
fn a_chat_no_process_holds_is_resumed() {
    assert_eq!(route(None), Route::Resume);
}

/// A live process with an inbox takes the words itself.
#[test]
fn a_live_process_with_an_inbox_takes_the_message() {
    let session = live(7, "s", Some("/tmp/cc-socks/7.sock"), Some(PEER_PROTOCOL));
    assert_eq!(route(Some(session.clone())), Route::Inbox(session));
}

/// A live process without an inbox is the one honest "busy".
#[test]
fn a_live_process_without_an_inbox_is_busy() {
    assert_eq!(
        route(Some(live(7, "s", None, Some(PEER_PROTOCOL)))),
        Route::Busy
    );
}

/// A protocol this code does not speak is a wait, never a second agent.
#[test]
fn an_unknown_peer_protocol_is_busy_not_a_resume() {
    for protocol in [None, Some(0), Some(2), Some(u64::MAX)] {
        assert_eq!(
            route(Some(live(7, "s", Some("/tmp/cc-socks/7.sock"), protocol))),
            Route::Busy,
            "{protocol:?}"
        );
    }
}

/// The registry on disk: records are `<pid>.json`, keys and junk sit next to
/// them, and a dead record is skipped.
#[test]
fn find_reads_pid_records_and_judges_them_by_the_caller() {
    let root = scratch("registry");
    let desktop = r#"{"pid":100,"sessionId":"chat","cwd":"/p","procStart":"Tue Sep  8 08:32:47 2026","version":"2.1.260","peerProtocol":1,"entrypoint":"claude-desktop","messagingSocketPath":"/tmp/cc-socks/100.sock"}"#;
    let stale = r#"{"pid":90,"sessionId":"chat","cwd":"/p","version":"2.1.260","peerProtocol":1,"messagingSocketPath":"/tmp/cc-socks/90.sock"}"#;
    let other = r#"{"pid":101,"sessionId":"elsewhere","cwd":"/p","version":"2.1.260"}"#;
    std::fs::write(root.join("100.json"), desktop).unwrap();
    std::fs::write(root.join("90.json"), stale).unwrap();
    std::fs::write(root.join("101.json"), other).unwrap();
    std::fs::write(root.join("100.abcdef.key"), "{\"peerToken\":\"x\"}").unwrap();
    std::fs::write(
        root.join("notes.json"),
        "{\"pid\":1,\"sessionId\":\"chat\"}",
    )
    .unwrap();
    std::fs::write(root.join("7.json"), "not json").unwrap();

    let alive = |s: &LiveSession| s.pid == 100;
    let found = find(&root, "chat", &alive).expect("the desktop process");
    assert_eq!(found.pid, 100);
    assert_eq!(found.entrypoint.as_deref(), Some("claude-desktop"));
    assert!(found.inbox.is_some());

    assert!(find(&root, "chat", &|_| false).is_none());
    assert!(find(&root, "nobody", &|_| true).is_none());
    // No registry at all reads as no process: the chat is free.
    assert!(find(Path::new("/nonexistent/peekle"), "chat", &|_| true).is_none());

    std::fs::remove_dir_all(&root).unwrap();
}

/// Two live records for one chat -- a resumer next to the original -- and
/// the one that can take words wins.
#[test]
fn among_live_records_the_one_with_an_inbox_wins() {
    let root = scratch("registry-two");
    let with = r#"{"pid":100,"sessionId":"chat","cwd":"/p","version":"2.1.260","peerProtocol":1,"messagingSocketPath":"/tmp/cc-socks/100.sock"}"#;
    let without =
        r#"{"pid":50,"sessionId":"chat","cwd":"/p","version":"2.1.220","peerProtocol":1}"#;
    std::fs::write(root.join("50.json"), without).unwrap();
    std::fs::write(root.join("100.json"), with).unwrap();

    let found = find(&root, "chat", &|_| true).unwrap();
    assert_eq!(found.pid, 100);

    std::fs::remove_dir_all(&root).unwrap();
}

fn arb_session() -> impl Strategy<Value = LiveSession> {
    (
        any::<u32>(),
        "[a-z0-9-]{1,40}",
        proptest::option::of("/tmp/cc-socks/[0-9]{1,6}\\.sock"),
        proptest::option::of(any::<u64>()),
    )
        .prop_map(|(pid, id, inbox, protocol)| live(pid, &id, inbox.as_deref(), protocol))
}

proptest! {
    /// Total, and a live process is never resumed over: the race of v34.
    #[test]
    fn a_live_record_never_routes_to_resume(session in arb_session()) {
        let decision = route(Some(session.clone()));
        prop_assert_ne!(decision.clone(), Route::Resume);
        let expected_inbox = session.inbox.is_some() && session.peer_protocol == Some(PEER_PROTOCOL);
        prop_assert_eq!(matches!(decision, Route::Inbox(_)), expected_inbox);
    }

    /// The parser survives anything and never invents a session.
    #[test]
    fn parsing_never_panics(text in "\\PC{0,300}") {
        if let Some(session) = parse_record(&text) {
            prop_assert!(!session.session_id.is_empty());
        }
    }
}
