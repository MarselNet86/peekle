#![allow(clippy::unwrap_used)]
//! S21 acceptance: the two lines that put a reply into a live process, sent
//! to an inbox stood up in this process, and every way that can fail short
//! of the words arriving. tech.md 6.5 and R-17.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use peekle_core::inbox::{key_name, send, InboxError};
use peekle_core::registry::{LiveSession, PEER_PROTOCOL};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// Under `/tmp` and short: a unix socket path must fit in about 104 bytes,
/// and the per-user temp dir on macOS eats half of that on its own.
fn scratch(name: &str) -> PathBuf {
    let id = ulid::Ulid::generate().to_string();
    let dir = PathBuf::from(format!("/tmp/pk-{name}-{}", &id[id.len() - 8..]));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn live(pid: u32, inbox: &std::path::Path) -> LiveSession {
    LiveSession {
        pid,
        session_id: "chat-1".to_string(),
        cwd: "/tmp/proj".to_string(),
        proc_start: None,
        version: "2.1.261".to_string(),
        entrypoint: Some("claude-desktop".to_string()),
        peer_protocol: Some(PEER_PROTOCOL),
        inbox: Some(inbox.to_path_buf()),
    }
}

/// An inbox the way the process publishes it: a socket, and a key file named
/// for it under the registry root. Returns the lines it received.
fn stand_up_inbox(
    root: &std::path::Path,
    pid: u32,
) -> (PathBuf, std::thread::JoinHandle<Vec<String>>) {
    let socket = root.join(format!("{pid}.sock"));
    std::fs::write(
        root.join(key_name(pid, &socket)),
        format!("{{\"peerToken\":\"{TOKEN}\",\"pidDomain\":\"darwin\"}}"),
    )
    .unwrap();
    let listener = UnixListener::bind(&socket).unwrap();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut lines = Vec::new();
        let mut reader = BufReader::new(stream);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => lines.push(line.trim_end_matches('\n').to_string()),
            }
        }
        // A receipt, the way a peer session answers. This end ignores it.
        let _ = reader.get_mut().write_all(b"{\"type\":\"receipt\"}\n");
        lines
    });
    (socket, handle)
}

#[test]
fn a_message_arrives_as_the_auth_line_then_the_turn() {
    let root = scratch("inbox");
    let pid = std::process::id();
    let (socket, inbox) = stand_up_inbox(&root, pid);

    send(&root, &live(pid, &socket), "make the tests green").unwrap();

    let lines = inbox.join().unwrap();
    assert_eq!(lines.len(), 2, "{lines:?}");
    let auth: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    let turn: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(auth["type"], "auth");
    assert_eq!(auth["token"], TOKEN);
    assert_eq!(turn["type"], "user");
    assert_eq!(turn["message"]["role"], "user");
    assert_eq!(turn["message"]["content"], "make the tests green");
    assert_eq!(turn["session_id"], "chat-1");
    assert!(turn["msg_id"].as_str().is_some_and(|id| !id.is_empty()));

    std::fs::remove_dir_all(&root).unwrap();
}

/// A record that publishes no socket has no way in.
#[test]
fn no_inbox_is_refused_before_anything_is_read() {
    let root = scratch("inbox-none");
    let mut session = live(1, std::path::Path::new("/nowhere.sock"));
    session.inbox = None;
    assert!(matches!(
        send(&root, &session, "x"),
        Err(InboxError::NoInbox)
    ));
    std::fs::remove_dir_all(&root).unwrap();
}

/// A socket with no key next to its record is unreachable, not unauthenticated.
#[test]
fn a_missing_key_is_refused_without_connecting() {
    let root = scratch("inbox-nokey");
    let socket = root.join("1.sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    assert!(matches!(
        send(&root, &live(1, &socket), "x"),
        Err(InboxError::NoKey)
    ));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_key_in_another_shape_is_refused() {
    let root = scratch("inbox-badkey");
    let socket = root.join("1.sock");
    for text in [
        "",
        "not json",
        "{}",
        "{\"peerToken\":\"short\"}",
        "{\"peerToken\":42}",
    ] {
        std::fs::write(root.join(key_name(1, &socket)), text).unwrap();
        assert!(
            matches!(send(&root, &live(1, &socket), "x"), Err(InboxError::BadKey)),
            "{text:?}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

/// A socket path nobody listens on -- the process died between the registry
/// read and the connect -- is a connect error, and the error names no token.
#[test]
fn a_dead_socket_is_a_connect_error() {
    let root = scratch("inbox-dead");
    let socket = root.join("1.sock");
    std::fs::write(
        root.join(key_name(1, &socket)),
        format!("{{\"peerToken\":\"{TOKEN}\"}}"),
    )
    .unwrap();
    let err = send(&root, &live(1, &socket), "x").unwrap_err();
    assert!(matches!(err, InboxError::Connect(_)), "{err}");
    assert!(!err.to_string().contains(TOKEN));
    std::fs::remove_dir_all(&root).unwrap();
}

/// The socket answers, but the process on it is not the one the record
/// names: a reused path. Nothing is written. macOS is where the peer pid can
/// be read, and the check exists there.
#[cfg(target_os = "macos")]
#[test]
fn a_socket_held_by_another_process_gets_nothing() {
    let root = scratch("inbox-wrongpid");
    let claimed = std::process::id() + 100_000;
    let (socket, inbox) = stand_up_inbox(&root, claimed);

    let err = send(&root, &live(claimed, &socket), "x").unwrap_err();
    assert!(
        matches!(err, InboxError::WrongPeer { expected, found } if expected == claimed && found == std::process::id()),
        "{err}"
    );
    // The listener saw a connection and no lines.
    let lines = inbox.join().unwrap();
    assert!(lines.is_empty(), "{lines:?}");

    std::fs::remove_dir_all(&root).unwrap();
}
