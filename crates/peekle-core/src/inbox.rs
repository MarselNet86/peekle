//! The way into a live `claude` process: the inbox it publishes itself.
//! tech.md 6.5 and R-17.
//!
//! Claude Code 2.1.26x opens a unix socket per process and takes newline
//! delimited JSON on it. The first line authenticates with the token from the
//! key file next to the process's registry record; the second is the message.
//! The binary prints this very recipe in its own log as the way to inject a
//! message with `socat`, so it is its contract, undocumented as it is.
//!
//! Nothing here logs, formats or returns the token. Rule 11.

#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::Path;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::registry::LiveSession;

/// How long a connect, a write or the final drain may take. The socket is
/// local; anything slower than this is a process that is not answering.
#[cfg(unix)]
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// What `Stop` says to a process that is not ours. A request, not an
/// interrupt: the inbox has no interrupt frame, and a signal would end the
/// process rather than the turn. Read at the next tool boundary, like a line
/// typed into a busy TUI. tech.md 6.5.
pub const STOP_REQUEST: &str =
    "Stop. End this turn now without running anything else, and say in one line where you left off.";

#[derive(Debug, thiserror::Error)]
pub enum InboxError {
    #[error("that session publishes no inbox")]
    NoInbox,
    #[error("no key for that inbox")]
    NoKey,
    #[error("the inbox key is not in a shape this version knows")]
    BadKey,
    #[error("could not reach the inbox: {0}")]
    Connect(String),
    #[error("the inbox belongs to another process (expected pid {expected}, found {found})")]
    WrongPeer { expected: u32, found: u32 },
    #[error("the inbox did not take the message: {0}")]
    Write(String),
    /// This platform has no inbox transport Peekle knows. Windows: Claude
    /// Code cannot open a unix socket there, and what it opens instead has
    /// not been captured, so nothing is faked. The ladder of 6.5 goes on to
    /// `--resume`. tech.md 6.27 and R-21.
    #[error("no inbox transport on this platform")]
    Unsupported,
}

/// The name of the key file for `pid`'s inbox at `inbox`: the pid, the
/// sha256 of the socket path, `.key`. Seen on disk as
/// `16806.66d3dd08….key` next to `/tmp/cc-socks/16806.sock`.
pub fn key_name(pid: u32, inbox: &Path) -> String {
    let digest = Sha256::digest(inbox.as_os_str().as_encoded_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{pid}.{hex}.key")
}

/// The key file's shape. No `Debug`: it holds the token.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyFile {
    peer_token: String,
}

/// The shape of a peer token. The rule holds on every platform, and its test
/// runs on every platform; only the caller is unix-only. tech.md 6.27.
#[cfg_attr(not(unix), allow(dead_code))]
fn is_token(token: &str) -> bool {
    token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The two lines that carry one message: the auth line, then the message.
/// `text` goes as it is, unescaped by anything but JSON.
pub fn frames(token: &str, session_id: &str, text: &str, msg_id: &str) -> [String; 2] {
    let auth = serde_json::json!({ "type": "auth", "token": token });
    let user = serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": text },
        "session_id": session_id,
        "msg_id": msg_id,
    });
    [auth.to_string(), user.to_string()]
}

/// Puts `text` into the inbox `live` publishes, as a turn of that session.
///
/// `sessions_root` is where the registry and its keys live, normally
/// `~/.claude/sessions`. The connected end is checked to be `live.pid`
/// before anything is written: a socket path outlives the process that
/// bound it, and the next process to bind there is not the chat.
#[cfg(unix)]
pub fn send(sessions_root: &Path, live: &LiveSession, text: &str) -> Result<(), InboxError> {
    let Some(inbox) = live.inbox.as_deref() else {
        return Err(InboxError::NoInbox);
    };
    let key_path = sessions_root.join(key_name(live.pid, inbox));
    let key_text = std::fs::read_to_string(&key_path).map_err(|_| InboxError::NoKey)?;
    let key: KeyFile = serde_json::from_str(&key_text).map_err(|_| InboxError::BadKey)?;
    if !is_token(&key.peer_token) {
        return Err(InboxError::BadKey);
    }

    let mut stream =
        UnixStream::connect(inbox).map_err(|err| InboxError::Connect(err.to_string()))?;
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));

    if let Some(found) = peer_pid(&stream) {
        if found != live.pid {
            return Err(InboxError::WrongPeer {
                expected: live.pid,
                found,
            });
        }
    }

    let msg_id = crate::pty::new_session_id();
    for line in frames(&key.peer_token, &live.session_id, text, &msg_id) {
        stream
            .write_all(line.as_bytes())
            .and_then(|()| stream.write_all(b"\n"))
            .map_err(|err| InboxError::Write(err.to_string()))?;
    }
    stream
        .flush()
        .map_err(|err| InboxError::Write(err.to_string()))?;
    let _ = stream.shutdown(std::net::Shutdown::Write);

    // Whatever the inbox says back is a receipt for a peer session to
    // correlate; this end has no outstanding sends to match it against. Read
    // until it hangs up so the close is orderly, and no longer.
    let mut sink = [0u8; 4096];
    while let Ok(n) = stream.read(&mut sink) {
        if n == 0 {
            break;
        }
    }
    Ok(())
}

/// Windows: no unix socket, no known transport. tech.md 6.27 and R-21.
#[cfg(not(unix))]
pub fn send(_sessions_root: &Path, _live: &LiveSession, _text: &str) -> Result<(), InboxError> {
    Err(InboxError::Unsupported)
}

/// The pid on the other end of a connected unix socket, where the platform
/// tells. macOS does, through `LOCAL_PEERPID`; elsewhere the check is skipped
/// rather than faked.
#[cfg(target_os = "macos")]
fn peer_pid(stream: &UnixStream) -> Option<u32> {
    use std::os::fd::AsRawFd;

    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // SAFETY: a plain getsockopt on a socket this process owns, into a
    // buffer of the size the option is documented to fill.
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            std::ptr::addr_of_mut!(pid).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return None;
    }
    u32::try_from(pid).ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn peer_pid(_stream: &UnixStream) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seen on this machine: the socket and the key next to its record.
    #[test]
    fn the_key_name_is_the_pid_and_the_hash_of_the_socket_path() {
        assert_eq!(
            key_name(16806, Path::new("/tmp/cc-socks/16806.sock")),
            "16806.66d3dd08e8c81ef2505c92ba59ca215d07c6f6fdc54353fa794de4839fb5fac5.key"
        );
    }

    #[test]
    fn the_auth_line_comes_first_and_the_message_carries_the_text_as_typed() {
        let [auth, user] = frames(
            "0123456789abcdef0123456789abcdef",
            "s-1",
            "fix \"it\"\nnow",
            "m-1",
        );
        let auth: serde_json::Value = serde_json::from_str(&auth).unwrap();
        let user: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(auth["type"], "auth");
        assert_eq!(auth["token"], "0123456789abcdef0123456789abcdef");
        assert_eq!(user["type"], "user");
        assert_eq!(user["message"]["role"], "user");
        assert_eq!(user["message"]["content"], "fix \"it\"\nnow");
        assert_eq!(user["session_id"], "s-1");
        assert_eq!(user["msg_id"], "m-1");
        assert!(!auth.to_string().contains('\n'));
        assert!(!user.to_string().contains('\n'));
    }

    #[test]
    fn a_token_is_thirty_two_hex_digits() {
        assert!(is_token("0123456789abcdef0123456789abcdef"));
        assert!(!is_token("0123456789abcdef0123456789abcde"));
        assert!(!is_token("0123456789abcdef0123456789abcdeg"));
        assert!(!is_token(""));
    }

    #[test]
    fn no_error_names_the_token() {
        for err in [
            InboxError::NoInbox,
            InboxError::NoKey,
            InboxError::BadKey,
            InboxError::Connect("gone".into()),
            InboxError::WrongPeer {
                expected: 1,
                found: 2,
            },
            InboxError::Write("closed".into()),
        ] {
            let text = format!("{err} {err:?}");
            assert!(!text.contains("0123456789abcdef"), "{text}");
        }
    }
}
