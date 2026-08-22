//! The session the island owns: `claude` running in a pty Peekle holds.
//!
//! This is the only channel text takes to an agent. tech.md 6.5 is the source
//! of truth; the short version is that there is no supported way to type into
//! a process we did not start, so we start it.
//!
//! A pty and not `-p`: the session stays interactive, so `PermissionRequest`
//! fires and the Allow/Deny gate survives. Print mode never raises it, which
//! is what made the old `--resume` path a channel that bypassed the product.
//!
//! Everything that touches a real process is behind [`PtyHost`]. The parts
//! that decide what bytes go on the wire are free functions, so they stay
//! testable without spawning anything.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

/// What a spawn needs to know. Kept separate from the pty so the argument
/// building can be tested without a `claude` binary on the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub session_id: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("no claude binary found")]
    NoBinary,
    #[error("working directory does not exist")]
    NoCwd,
    #[error("session is not one the island owns")]
    NotOwned,
    #[error("pty failed: {0}")]
    Pty(String),
}

/// The arguments `claude` is started with.
///
/// `--session-id` is the whole trick: it tells us up front which `session_id`
/// the hooks will carry, so a hook is matched to a card without a guess, and
/// anything else is an observed session. tech.md 6.5.
pub fn spawn_args(session_id: &str) -> Vec<String> {
    vec!["--session-id".to_string(), session_id.to_string()]
}

/// The bytes one message puts on the wire.
///
/// The newline is a separate write for the same reason Enter was a separate
/// `send-keys` command: inside one write with a multi-line body it cannot be
/// told apart from the newlines the user typed. The text itself goes as-is,
/// with no parsing and no escaping — a pty is a stream, not a list of key
/// names, so the whole `send-keys` quoting problem does not exist here.
pub fn message_writes(text: &str) -> Vec<Vec<u8>> {
    vec![text.as_bytes().to_vec(), b"\r".to_vec()]
}

/// A session id Claude Code accepts: it insists on a UUID.
pub fn new_session_id() -> String {
    let raw = ulid::Ulid::generate().to_bytes();
    let hex: String = raw.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Where tools live when the shell never ran. Used only as the fallback, and
/// the same reasoning as `claude_path`: these are where a Mac keeps them.
const PATH_FALLBACK: &[&str] = &[
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

/// The `PATH` an owned session runs with.
///
/// Peekle is launched by Finder, so it inherits a login environment with
/// almost nothing on the path. An agent started from there would not find the
/// tools the user has in their own terminal — no `pnpm`, no `cargo`, no
/// anything they installed themselves — and would fail at work that succeeds
/// when they run it by hand. Asking the login shell is the only way to get the
/// path the user actually means, so ask it once and remember the answer.
///
/// `-l` alone is not enough: people set `PATH` in `.zshrc`, which only an
/// interactive shell reads. A shell that hangs or prints nothing falls back to
/// the static list rather than leaving the session with no path at all.
fn session_path() -> String {
    static PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        let fallback = || PATH_FALLBACK.join(":");
        let Ok(shell) = std::env::var("SHELL") else {
            return fallback();
        };
        let output = std::process::Command::new(shell)
            .args(["-lic", "printf %s \"$PATH\""])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if path.is_empty() {
                    fallback()
                } else {
                    path
                }
            }
            other => {
                tracing::warn!(?other, "could not read the login PATH");
                fallback()
            }
        }
    })
    .clone()
}

/// One live session and the handles that keep it alive.
struct Owned {
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Held so the pty is not closed while the reader thread drains it.
    _master: Box<dyn portable_pty::MasterPty + Send>,
}

/// The sessions Peekle started, by `session_id`.
#[derive(Default)]
pub struct PtyHost {
    sessions: Mutex<HashMap<String, Owned>>,
}

impl PtyHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this id belongs to a session we started.
    pub fn owns(&self, session_id: &str) -> bool {
        self.lock().contains_key(session_id)
    }

    pub fn owned_ids(&self) -> Vec<String> {
        self.lock().keys().cloned().collect()
    }

    /// Starts `claude` in a fresh pty.
    ///
    /// `on_exit` runs on the reader thread when the process goes away, which
    /// is the only moment `SessionStatus::Ended` is a fact rather than a
    /// guess: the process was ours. tech.md 6.3.
    pub fn spawn<F>(&self, binary: &Path, spec: &SpawnSpec, on_exit: F) -> Result<(), PtyError>
    where
        F: FnOnce(String) + Send + 'static,
    {
        if !Path::new(&spec.cwd).is_dir() {
            return Err(PtyError::NoCwd);
        }

        let system = NativePtySystem::default();
        let pair = system
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| PtyError::Pty(err.to_string()))?;

        let mut command = CommandBuilder::new(binary);
        for arg in spawn_args(&spec.session_id) {
            command.arg(arg);
        }
        command.cwd(&spec.cwd);
        // An interactive session needs a terminal type; a GUI process has none
        // to inherit, and Claude Code draws a TUI.
        command.env("TERM", "xterm-256color");
        command.env("PATH", session_path());

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|err| PtyError::Pty(err.to_string()))?;
        // The slave end is done once the child holds it; keeping it open here
        // would stop us ever seeing EOF on the master.
        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|err| PtyError::Pty(err.to_string()))?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| PtyError::Pty(err.to_string()))?;

        // Drain and discard. The feed is built from hooks and transcripts, not
        // from the TUI, but an unread buffer fills up and wedges the process on
        // the other end, so reading is not optional. tech.md 6.5.
        let id = spec.session_id.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
            }
            on_exit(id);
        });

        self.lock().insert(
            spec.session_id.clone(),
            Owned {
                writer,
                child,
                _master: pair.master,
            },
        );
        Ok(())
    }

    /// Writes one message into a session's pty.
    ///
    /// Refuses anything we do not own rather than dropping it silently: an
    /// observed session has no input field, and a swallowed message would be
    /// exactly the lie the field is meant not to tell.
    pub fn send(&self, session_id: &str, text: &str) -> Result<(), PtyError> {
        let mut sessions = self.lock();
        let owned = sessions.get_mut(session_id).ok_or(PtyError::NotOwned)?;
        for chunk in message_writes(text) {
            owned
                .writer
                .write_all(&chunk)
                .map_err(|err| PtyError::Pty(err.to_string()))?;
        }
        owned
            .writer
            .flush()
            .map_err(|err| PtyError::Pty(err.to_string()))
    }

    /// Ends a session Peekle owns. Unknown ids are a no-op: the process may
    /// have exited on its own a moment earlier, and that is not an error.
    pub fn end(&self, session_id: &str) {
        if let Some(mut owned) = self.lock().remove(session_id) {
            let _ = owned.child.kill();
        }
    }

    /// Forgets a session whose process exited by itself.
    pub fn forget(&self, session_id: &str) {
        self.lock().remove(session_id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Owned>> {
        // A poisoned lock means another thread panicked holding it. The map is
        // still structurally sound, and refusing to type from then on would be
        // worse than carrying on.
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Shared handle, because commands and the hook sink both reach for it.
pub type SharedPtyHost = Arc<PtyHost>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_the_session_id_it_assigned() {
        assert_eq!(
            spawn_args("abc"),
            vec!["--session-id".to_string(), "abc".to_string()]
        );
    }

    #[test]
    fn newline_is_its_own_write() {
        let writes = message_writes("build it");
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], b"build it");
        assert_eq!(writes[1], b"\r");
    }

    /// The class of bug `send-keys` had without `-l`: the word Enter, braces,
    /// quotes and unicode all have to arrive as themselves.
    #[test]
    fn text_goes_byte_for_byte() {
        for text in [
            "Enter",
            "press Enter then C-c",
            "say \"hi\"; echo $HOME `date`",
            "{ \"a\": [1, 2] }",
            "печатай юникод 🚀",
            "line one\nline two",
        ] {
            let writes = message_writes(text);
            assert_eq!(writes[0], text.as_bytes(), "text mangled: {text}");
        }
    }

    #[test]
    fn session_id_is_a_uuid() {
        let id = new_session_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|part| part.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12],
            "not uuid shaped: {id}"
        );
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        assert_ne!(id, new_session_id());
    }

    #[test]
    fn refuses_a_session_it_does_not_own() {
        let host = PtyHost::new();
        assert!(!host.owns("nope"));
        assert!(matches!(host.send("nope", "text"), Err(PtyError::NotOwned)));
    }

    #[test]
    fn ending_an_unknown_session_is_a_no_op() {
        let host = PtyHost::new();
        host.end("nope");
        host.forget("nope");
    }

    /// The session must not run with the crippled path a GUI app inherits, or
    /// the agent cannot find the tools the user works with.
    #[test]
    fn the_session_path_is_never_empty() {
        let path = session_path();
        assert!(!path.is_empty());
        assert!(
            path.split(':').any(|dir| dir == "/usr/bin"),
            "a usable path at minimum: {path}"
        );
    }

    #[test]
    fn refuses_a_working_directory_that_is_not_there() {
        let host = PtyHost::new();
        let spec = SpawnSpec {
            session_id: new_session_id(),
            cwd: "/nowhere/at/all".to_string(),
            cols: 120,
            rows: 40,
        };
        let result = host.spawn(Path::new("/bin/echo"), &spec, |_| {});
        assert!(matches!(result, Err(PtyError::NoCwd)));
    }

    /// The whole point of owning the process: we learn it died, and `Ended`
    /// becomes a fact instead of a guess.
    #[test]
    fn reports_the_exit_of_a_process_it_started() {
        let host = PtyHost::new();
        let spec = SpawnSpec {
            session_id: new_session_id(),
            cwd: "/tmp".to_string(),
            cols: 120,
            rows: 40,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        host.spawn(Path::new("/bin/echo"), &spec, move |id| {
            let _ = tx.send(id);
        })
        .expect("spawn echo");

        assert!(host.owns(&spec.session_id));
        let exited = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("exit reported");
        assert_eq!(exited, spec.session_id);
    }
}
