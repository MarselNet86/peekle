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
    /// Whether this spawn continues the chat `session_id` already names.
    ///
    /// A resumed session keeps its own id, so it is not assigned one: the id
    /// is what `--resume` is given. Continuing in place rather than forking is
    /// the point -- one chat, one transcript, and every other client watching
    /// that id sees the turns Peekle adds. tech.md 6.5.
    pub resume: bool,
    /// The chat to copy into `session_id`, when this spawn is a fork.
    ///
    /// The one case `--resume` in place cannot serve: another client holds
    /// that chat and takes no messages, so continuing in place would put two
    /// processes with two histories into one transcript, which is exactly
    /// what the CLI documents as interleaving. A fork copies the conversation
    /// under a new id and leaves the original alone. Measured live on 2.1.263:
    /// `--resume <old> --fork-session --session-id <new>` answers questions
    /// about the old conversation and writes only to the new transcript.
    /// tech.md 6.5.
    pub fork_from: Option<String>,
    /// The first message, handed to the spawn rather than typed into it.
    ///
    /// A freshly started TUI is not ready to receive a line for many seconds,
    /// and a line written before it is ready disappears without a trace
    /// (tech.md 6.15, v46.2). As an argument the prompt cannot be missed: the
    /// agent runs it as its first turn. tech.md 6.5.
    pub prompt: Option<String>,
    /// What the session answers with, picked before it ever ran. Flags rather
    /// than `/model` lines typed after the prompt: the prompt is the first
    /// turn, and a line typed behind it would apply to the second.
    /// tech.md 6.15.
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Which permission mode to start in, as `--permission-mode` takes it.
    /// tech.md 6.19.
    pub mode: Option<String>,
    /// Whether to start with thinking on. `Some(false)` puts
    /// `MAX_THINKING_TOKENS=0` in the environment, which is the CLI's own way
    /// of turning it off; `None` leaves the user's own settings alone.
    /// tech.md 6.20.
    pub thinking: Option<bool>,
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
///
/// With `resume`, the same three flags Claude Desktop uses to open an existing
/// chat: `--resume=<old>` continues that conversation, `--fork-session` gives
/// this run its own id so the fork does not collide with the original and its
/// hooks are recognised as ours, and `--session-id=<new>` is that id. The
/// original transcript is left untouched; the fork grows in a new file.
/// tech.md 6.5.
pub fn spawn_args(
    session_id: &str,
    resume: bool,
    fork_from: Option<&str>,
    prompt: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
    mode: Option<&str>,
) -> Vec<String> {
    // Three ways to end up with a session of a known id: assign one to a
    // fresh run, resume the chat that already has it, or copy another chat
    // into it. The CLI refuses `--session-id` beside `--resume` in the second
    // case and takes all three flags together in the third, which is what
    // makes a fork addressable: without `--session-id` the CLI picks the new
    // id and Peekle would not know where the conversation went. tech.md 6.5.
    let mut args = match (fork_from, resume) {
        (Some(from), _) => vec![
            "--resume".to_string(),
            from.to_string(),
            "--fork-session".to_string(),
            "--session-id".to_string(),
            session_id.to_string(),
        ],
        (None, true) => vec!["--resume".to_string(), session_id.to_string()],
        (None, false) => vec!["--session-id".to_string(), session_id.to_string()],
    };
    // What was picked before the first turn rides as flags, ahead of the
    // prompt: `--model` and `--effort` are the CLI's own, and they take effect
    // before the first request goes out. tech.md 6.15.
    for (flag, value) in [
        ("--model", model),
        ("--effort", effort),
        // The only exact way to set the permission mode: the CLI has no slash
        // command for it, and cycling `Shift+Tab` blind through a permission
        // setting is not a thing to do on someone's behalf. tech.md 6.19.
        ("--permission-mode", mode),
    ] {
        if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
            args.push(flag.to_string());
            args.push(value.to_string());
        }
    }
    // Positional, and last: it is the prompt, not a flag. Handed over rather
    // than typed, so the startup of the TUI cannot swallow it.
    if let Some(prompt) = prompt.map(str::trim).filter(|text| !text.is_empty()) {
        args.push(prompt.to_string());
    }
    args
}

/// The bytes one message puts on the wire.
///
/// The newline is a separate write for the same reason Enter was a separate
/// `send-keys` command: inside one write with a multi-line body it cannot be
/// told apart from the newlines the user typed. The text itself goes as-is,
/// with no parsing and no escaping — a pty is a stream, not a list of key
/// names, so the whole `send-keys` quoting problem does not exist here.
///
/// Separate in time too, by [`ENTER_GAP`]. See there for why.
pub fn message_writes(text: &str) -> Vec<Vec<u8>> {
    vec![text.as_bytes().to_vec(), b"\r".to_vec()]
}

/// How long the newline waits behind the text.
///
/// Two writes with nothing between them arrive as one read, and the TUI reads
/// bulk input as a paste — a carriage return inside a paste is not a send. The
/// text lands in the input box and stays there until some later write pulls the
/// TUI out of paste mode, which is the bug where the first message only goes
/// when the second is typed.
///
/// Measured against a live session through the transcript: a short reply is
/// submitted with no gap at all, a long one is never submitted, and 50ms is
/// enough. 120ms is clear of that and invisible to a person.
///
/// tmux got this for free, because `send-keys Enter` was a separate process.
/// tech.md 6.5.
pub const ENTER_GAP: std::time::Duration = std::time::Duration::from_millis(120);

/// How long a setting written ahead of a message waits before the message.
///
/// `/model` and `/effort` are local commands: the TUI runs one and redraws
/// before it is ready for the next line. The reply is what carries the
/// setting up to a session that has not spoken yet (tech.md 6.15), so the two
/// travel together, and this is the distance between them.
pub const SETTING_GAP: std::time::Duration = std::time::Duration::from_millis(400);

/// The bytes one slash command puts on the wire: the line, its newline, and a
/// second newline behind that.
///
/// A setting the CLI cannot apply silently asks about it instead.
/// `/effort ultracode` on a conversation that is already cached draws
/// `Change effort level?` with `Yes, switch to xhigh` under the cursor, and
/// the TUI stays modal until something answers. Everything written while it
/// stands belongs to the dialog: measured on a live 2.1.263 with Opus 5, the
/// message typed next was swallowed whole and its own newline answered the
/// question, so the words never reached the agent and nothing said so.
///
/// So the command answers its own question. Enter takes the option under the
/// cursor, which is the change that was just asked for; on a command that
/// raised no dialog it lands on an empty input box, where a newline does
/// nothing at all -- the same ground `NUDGE` stands on. tech.md 6.15.
pub fn command_writes(line: &str) -> Vec<Vec<u8>> {
    vec![line.as_bytes().to_vec(), b"\r".to_vec(), b"\r".to_vec()]
}

/// Whether this text is a slash command rather than something to say.
///
/// One line, a leading slash, and a first word that reads like a command
/// name: letters, digits and the punctuation a command name uses. A path is
/// not one -- `/Users/dev/peekle` carries slashes and dots and is an ordinary
/// thing to send an agent. The answer decides one thing only, whether the
/// write carries the newline that answers a dialog, and that newline is
/// harmless on an empty box, so a wrong guess here costs nothing either way.
/// tech.md 6.15.
pub fn is_slash_command(text: &str) -> bool {
    let line = text.trim();
    if line.lines().count() != 1 {
        return false;
    }
    let Some(rest) = line.strip_prefix('/') else {
        return false;
    };
    let Some(name) = rest.split_whitespace().next() else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
}

/// How long the answering newline waits behind a slash command.
///
/// The TUI has to run the command and draw what it asks before there is
/// anything to answer. The same 400ms that already separates one setting from
/// the next: measured against a live session, the dialog is up well inside
/// it, and the message that follows lands in the box the way it should.
pub const CONFIRM_GAP: std::time::Duration = SETTING_GAP;

/// What the CLI puts on screen when it asks whether this folder is trusted.
///
/// Captured live 2026-09-11 on 2.1.263 (`fixtures/pty/trust-question.txt`) by
/// starting a chat in a folder whose `hasTrustDialogAccepted` is false: a
/// full-screen question headed `Quick safety check`, with `No, exit` under the
/// cursor and `Yes, I trust this folder` below it. Until it is answered the
/// CLI runs nothing at all -- not the prompt it was handed, and not one hook
/// -- so a chat started in such a folder sat there in silence until its reply
/// went red. tech.md 6.24.
///
/// The mark carries no spaces because the screen has none: the TUI writes each
/// word and then jumps the cursor to the next column, so `Yes, I trust this
/// folder` reaches the pty as words with escape sequences between them and the
/// phrase never appears contiguously. [`asks_trust`] squeezes both out before
/// looking, which is also why the mark is this long: a single word survives
/// squeezing in any prose the agent might print.
/// Both of the answers it offers, because one of them is a sentence a person
/// could also type: the agent's own replies are drawn on this same screen, and
/// `I trust this folder` in a reply must not be read as the CLI asking.
pub const TRUST_MARKS: [&str; 2] = ["Itrustthisfolder", "No,exit"];

/// How long after a start the question can still be the question.
///
/// It is asked before anything runs and answered before anything else can
/// happen, so a screen that says it an hour in is the agent talking. tech.md
/// 6.24.
pub const TRUST_WINDOW: std::time::Duration = std::time::Duration::from_secs(120);

/// Whether this screen is the folder question. tech.md 6.24.
pub fn asks_trust(screen: &str) -> bool {
    let screen = squeeze(screen);
    TRUST_MARKS.iter().all(|mark| screen.contains(mark))
}

/// A screen with its escape sequences and its whitespace taken out.
///
/// Terminal output is a drawing, not a document: the same sentence arrives
/// with cursor moves inside it, split across reads, and redrawn a dozen times.
/// What survives all of that is the order of the printed characters.
/// The most the trust-scan window may hold, and how much of it survives a trim.
///
/// Only enough to outlive one split mark has to be kept, and the marks are
/// short ASCII, so eight kibibytes is far more than any split can straddle.
const SCAN_CAP: usize = 16_384;
const SCAN_KEEP: usize = 8_192;

/// Trims the scan window without ever splitting a glyph.
///
/// The window is a `String`, and the screen it holds is full of multibyte
/// glyphs -- box drawing and Cyrillic. Cutting it by raw byte length
/// (`len - SCAN_KEEP`) panicked the reader the instant that offset fell inside
/// one of them, and a panicked reader stops draining the pty: the process on
/// the other end then wedges on a full buffer, and every reply typed into that
/// session sits unconfirmed. So the cut is walked forward to the next char
/// boundary, which `len` itself always is. tech.md 6.24.
fn trim_scan_window(tail: &mut String) {
    if tail.len() <= SCAN_CAP {
        return;
    }
    let mut cut = tail.len() - SCAN_KEEP;
    while cut < tail.len() && !tail.is_char_boundary(cut) {
        cut += 1;
    }
    tail.drain(..cut);
}

fn squeeze(screen: &str) -> String {
    let mut out = String::with_capacity(screen.len());
    let mut chars = screen.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // CSI and OSC both end on a byte outside the parameter range; the
            // two-character escapes end on the character right after it.
            match chars.peek() {
                Some('[') | Some(']') | Some('(') | Some(')') => {
                    chars.next();
                }
                _ => {
                    chars.next();
                    continue;
                }
            }
            for ch in chars.by_ref() {
                if ch.is_ascii_alphabetic() || ch == '\u{7}' || ch == '\u{9c}' {
                    break;
                }
            }
            continue;
        }
        if ch.is_whitespace() || ch.is_control() {
            continue;
        }
        out.push(ch);
    }
    out
}

/// The keys that answer it with yes.
///
/// The cursor stands on `No, exit`, so yes is one step down and Enter. Two
/// writes with a gap between them, for the reason [`ENTER_GAP`] gives: a
/// selection and its confirmation read as one keystroke when they arrive in
/// one read.
pub fn trust_writes() -> Vec<Vec<u8>> {
    vec![b"\x1b[B".to_vec(), b"\r".to_vec()]
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
pub(crate) fn session_path() -> String {
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

/// The `CLAUDE_CODE_*` variables Peekle inherited from whatever launched it.
///
/// A session the island starts is a session of its own, and it must not be
/// told it is a child of some other one. Launch Peekle from inside a Claude
/// Code session -- which a developer working on Peekle does constantly -- and
/// the environment carries `CLAUDE_CODE_SESSION_ID`, which argues with the
/// `--session-id` we pass, and `CLAUDE_CODE_CHILD_SESSION`, which switches
/// transcript saving off and takes the backfill of 6.11 down with it. Hooks
/// then arrive under an id that matches no card, so a reply sits unconfirmed
/// forever and the answer lands somewhere the user is not looking.
///
/// Scrubbed by prefix rather than by a list of names: the set of markers is
/// Claude Code's business and changes between versions, and inheriting a new
/// one would fail the same silent way.
fn is_session_marker(key: &str) -> bool {
    key.starts_with("CLAUDE_CODE_")
}

fn inherited_session_markers() -> Vec<String> {
    std::env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .filter(|key| is_session_marker(key))
        .collect()
}

/// One live session and the handles that keep it alive.
struct Owned {
    /// Behind its own lock, and not the map's.
    ///
    /// A write is a sequence with gaps in it -- text, `ENTER_GAP`, newline,
    /// and for a command `CONFIRM_GAP` and one more -- and the sequence has
    /// to be atomic per session or a second reply wedges between a message
    /// and its own newline. Holding the map for that half second would make
    /// every other session wait on it too, including one whose turn somebody
    /// is trying to interrupt. tech.md 6.5.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Held so the pty is not closed while the reader thread drains it.
    _master: Box<dyn portable_pty::MasterPty + Send>,
}

/// The writer of one session, with the map released before anything is
/// written. tech.md 6.5.
type Writer = Arc<Mutex<Box<dyn Write + Send>>>;

/// One chunk out to a session, flushed on its own.
///
/// Every write is flushed where it is written: two chunks left in the buffer
/// arrive as one read however long the gap between them was, and the gap is
/// the whole mechanism. tech.md 6.5.
fn put(writer: &mut (dyn Write + Send), chunk: &[u8]) -> Result<(), PtyError> {
    writer
        .write_all(chunk)
        .map_err(|err| PtyError::Pty(err.to_string()))?;
    writer.flush().map_err(|err| PtyError::Pty(err.to_string()))
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
    pub fn spawn<F, T>(
        &self,
        binary: &Path,
        spec: &SpawnSpec,
        on_exit: F,
        on_trust: T,
    ) -> Result<(), PtyError>
    where
        F: FnOnce(String) + Send + 'static,
        T: FnOnce(String) + Send + 'static,
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
        for arg in spawn_args(
            &spec.session_id,
            spec.resume,
            spec.fork_from.as_deref(),
            spec.prompt.as_deref(),
            spec.model.as_deref(),
            spec.effort.as_deref(),
            spec.mode.as_deref(),
        ) {
            command.arg(arg);
        }
        command.cwd(&spec.cwd);
        // An interactive session needs a terminal type; a GUI process has none
        // to inherit, and Claude Code draws a TUI.
        command.env("TERM", "xterm-256color");
        // The CLI names this itself, in the message it prints when an effort
        // needs thinking: "unset MAX_THINKING_TOKENS=0". Zero is off; leaving
        // it alone is whatever the user's settings say. tech.md 6.20.
        if spec.thinking == Some(false) {
            command.env("MAX_THINKING_TOKENS", "0");
        }
        command.env("PATH", session_path());
        for key in inherited_session_markers() {
            command.env_remove(key);
        }

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

        // Drained because an unread buffer wedges the process on the other
        // end, and read for exactly one thing: the question about trusting
        // this folder. The feed is still built from hooks and transcripts and
        // never from the TUI -- this is not feed, it is the one question that
        // stops every hook from ever arriving, so nothing but the screen can
        // report it. tech.md 6.5 and 6.24.
        let id = spec.session_id.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            // A window over the last two chunks, so the mark is still found
            // when a read splits it in half.
            let mut tail = String::new();
            let mut ask = Some(on_trust);
            let started = std::time::Instant::now();
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                if ask.is_some() && started.elapsed() < TRUST_WINDOW {
                    tail.push_str(&squeeze(&String::from_utf8_lossy(&buffer[..read])));
                    if TRUST_MARKS.iter().all(|mark| tail.contains(mark)) {
                        if let Some(ask) = ask.take() {
                            ask(id.clone());
                        }
                    }
                    // Screens are redrawn whole and often; this only has to
                    // outlive one split mark.
                    trim_scan_window(&mut tail);
                }
            }
            on_exit(id);
        });

        self.lock().insert(
            spec.session_id.clone(),
            Owned {
                writer: Arc::new(Mutex::new(writer)),
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
    /// The gap is held under this lock on purpose: released between the two
    /// writes, a second reply would wedge between the first one's text and its
    /// newline, and the agent would receive the two glued together.
    pub fn send(&self, session_id: &str, text: &str) -> Result<(), PtyError> {
        let writer = self.writer_of(session_id)?;
        let mut writer = lock_writer(&writer);

        for (index, chunk) in message_writes(text).into_iter().enumerate() {
            // Each half is flushed on its own, or the gap buys nothing: both
            // would still reach the far end in a single read.
            if index > 0 {
                std::thread::sleep(ENTER_GAP);
            }
            put(writer.as_mut(), &chunk)?;
        }
        Ok(())
    }

    /// Writes one slash command, and answers the question it may raise.
    ///
    /// The same channel a message takes, with one more newline behind it: a
    /// setting the CLI cannot apply silently puts a dialog on screen instead,
    /// and until something answers, everything written next is eaten by it --
    /// including the next thing the person types. See [`command_writes`].
    /// tech.md 6.15.
    pub fn command(&self, session_id: &str, line: &str) -> Result<(), PtyError> {
        let writer = self.writer_of(session_id)?;
        let mut writer = lock_writer(&writer);

        for (index, chunk) in command_writes(line).into_iter().enumerate() {
            match index {
                0 => {}
                1 => std::thread::sleep(ENTER_GAP),
                _ => std::thread::sleep(CONFIRM_GAP),
            }
            put(writer.as_mut(), &chunk)?;
        }
        Ok(())
    }

    /// Ends a session Peekle owns. Unknown ids are a no-op: the process may
    /// have exited on its own a moment earlier, and that is not an error.
    /// Answers the folder trust question with yes. tech.md 6.24.
    ///
    /// Only ever called because the person answered it in the island: the CLI
    /// asks whether they vouch for what is in the folder, and that is not a
    /// question an overlay may answer for them.
    pub fn trust(&self, session_id: &str) -> Result<(), PtyError> {
        let writer = self.writer_of(session_id)?;
        let mut writer = lock_writer(&writer);
        for (index, chunk) in trust_writes().into_iter().enumerate() {
            if index > 0 {
                std::thread::sleep(ENTER_GAP);
            }
            put(writer.as_mut(), &chunk)?;
        }
        Ok(())
    }

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

/// The one byte that interrupts a turn: `Esc`, the key the TUI binds to it.
/// No newline follows -- it is a key, not a line -- and no gap either, since
/// there is nothing for a gap to separate. tech.md 6.5.
pub const INTERRUPT: &[u8] = b"\x1b";

/// One newline and nothing else: submit whatever is in the input box.
///
/// A message is written as text, a pause, then this. When the TUI reads both
/// halves in one go it takes them for a paste, and a newline inside a paste is
/// not a send -- the text then sits in the box, typed and unsent, and the turn
/// never starts. `ENTER_GAP` makes that rare rather than impossible: the gap is
/// wall clock, and the far end has to be scheduled inside it to see two reads.
/// So the reply that nothing confirmed is nudged with one of these before it is
/// given up on. Harmless on an empty box, which is what it finds when the text
/// did go. tech.md 6.5.
pub const NUDGE: &[u8] = b"\r";

/// `Shift+Tab`, the key the TUI binds to cycling the permission mode. CSI Z,
/// the terminal's back-tab. tech.md 6.19.
pub const CYCLE_MODE: &[u8] = b"\x1b[Z";

/// How long the TUI is given to redraw between two presses.
///
/// Measured on a live session: presses land as separate keys well inside a
/// second. A gap that is too small risks two presses read as one, and the
/// cost of being wrong is a session left in a mode nobody chose, so this is
/// generous rather than tight. tech.md 6.19.
pub const CYCLE_GAP: std::time::Duration = std::time::Duration::from_millis(250);

impl PtyHost {
    /// Steps the permission mode `steps` places along the cycle.
    ///
    /// One key per step, spaced, exactly as a person would press it. There is
    /// nothing else to write: the CLI has no line for the mode. Sending zero
    /// keys is not an error -- it is what asking for the mode you are already
    /// in means. tech.md 6.19.
    pub fn cycle_mode(&self, session_id: &str, steps: usize) -> Result<(), PtyError> {
        for step in 0..steps {
            if step > 0 {
                std::thread::sleep(CYCLE_GAP);
            }
            self.write_bytes(session_id, CYCLE_MODE)?;
        }
        Ok(())
    }

    /// Submits whatever the input box holds, without adding to it. tech.md 6.5.
    pub fn nudge(&self, session_id: &str) -> Result<(), PtyError> {
        self.write_bytes(session_id, NUDGE)
    }

    /// Interrupts the running turn of a session Peekle owns. tech.md 6.5.
    pub fn interrupt(&self, session_id: &str) -> Result<(), PtyError> {
        self.write_bytes(session_id, INTERRUPT)
    }

    /// One write, under the same lock as `send`, so nothing lands between a
    /// message's text and its own newline.
    fn write_bytes(&self, session_id: &str, bytes: &[u8]) -> Result<(), PtyError> {
        let writer = self.writer_of(session_id)?;
        let mut writer = lock_writer(&writer);
        put(writer.as_mut(), bytes)
    }

    /// The writer of one session. The map is released as this returns, so a
    /// write into one session never holds up a write into another -- least of
    /// all an interrupt. tech.md 6.5.
    fn writer_of(&self, session_id: &str) -> Result<Writer, PtyError> {
        let sessions = self.lock();
        let owned = sessions.get(session_id).ok_or(PtyError::NotOwned)?;
        Ok(Arc::clone(&owned.writer))
    }
}

/// A poisoned writer means a thread panicked mid-write. The stream is still a
/// stream; refusing to type from then on would be worse than carrying on.
fn lock_writer(writer: &Writer) -> std::sync::MutexGuard<'_, Box<dyn Write + Send>> {
    writer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Shared handle, because commands and the hook sink both reach for it.
pub type SharedPtyHost = Arc<PtyHost>;

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/pty")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    /// tech.md 6.24. The question is read off the screen because nothing else
    /// reports it, so the only test worth anything is the screen itself.
    #[test]
    fn the_folder_question_is_recognised_on_the_screen_that_asks_it() {
        assert!(asks_trust(&screen("trust-question.txt")));
        assert!(!asks_trust(&screen("no-question.txt")));
    }

    /// And the reason the mark has no spaces in it. The TUI draws the question
    /// a word at a time, jumping the cursor between them, and only prints the
    /// row as a sentence when it redraws it under the cursor. A matcher that
    /// looked for the sentence would wait for that redraw; this one has the
    /// question before it comes.
    /// The reader used to trim its scan window by raw byte length, and the
    /// TUI is full of multibyte glyphs -- the box drawing of its frames and
    /// the Cyrillic a person types. When the cut fell inside one, the reader
    /// panicked and stopped draining the pty; the process wedged on a full
    /// buffer and every reply into that session sat unconfirmed. So the trim
    /// must land on a char boundary, whatever the window holds. tech.md 6.24.
    #[test]
    fn the_scan_window_is_trimmed_without_splitting_a_glyph() {
        // A window well over the cap, made of two-byte glyphs so that
        // `len - SCAN_KEEP` lands mid-glyph as often as not.
        let mut tail = "⏵ добавь поддержку отправки ".repeat(1_500);
        assert!(tail.len() > SCAN_CAP);
        trim_scan_window(&mut tail); // the call that used to panic
        assert!(tail.len() <= SCAN_CAP, "the window was actually trimmed");
        assert!(tail.is_char_boundary(0) && std::str::from_utf8(tail.as_bytes()).is_ok());

        // A short ASCII mark riding the very end of the window still survives a
        // trim, so the question is not lost to it.
        let mut tail = "ф".repeat(9_000);
        tail.push_str("No,exit");
        trim_scan_window(&mut tail);
        assert!(tail.contains("No,exit"), "the mark at the tail survived");
    }

    /// Nothing to trim is left alone, byte-for-byte.
    #[test]
    fn a_small_scan_window_is_untouched() {
        let mut tail = "Yes,Itrustthisfolder No,exit".to_string();
        let before = tail.clone();
        trim_scan_window(&mut tail);
        assert_eq!(tail, before);
    }

    #[test]
    fn the_question_is_caught_on_its_first_drawing() {
        let raw = screen("trust-question.txt");
        let whole = raw
            .find("I trust this folder")
            .expect("a later redraw prints the row whole");
        let first_drawing = &raw[..whole];

        assert!(!first_drawing.contains("I trust this folder"));
        assert!(
            asks_trust(first_drawing),
            "the question is on screen well before it is on screen as a sentence"
        );
    }

    /// One answer is a sentence somebody could type, so it is not enough on
    /// its own: the agent's own replies are drawn on this same screen.
    #[test]
    fn a_reply_that_says_the_same_words_is_not_the_question() {
        assert!(!asks_trust("Sure, I trust this folder -- go ahead"));
        // Both halves, and then it is the question and nothing else.
        assert!(asks_trust("No, exit\nYes, I trust this folder"));
    }

    /// The cursor stands on `No, exit`, so yes is a move and then a
    /// confirmation -- in that order, and never in one write. tech.md 6.24.
    #[test]
    fn yes_is_a_move_and_then_a_confirmation() {
        let writes = trust_writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], b"\x1b[B".to_vec(), "one step down, onto yes");
        assert_eq!(writes[1], b"\r".to_vec());
    }

    #[test]
    fn passes_the_session_id_it_assigned() {
        assert_eq!(
            spawn_args("abc", false, None, None, None, None, None),
            vec!["--session-id".to_string(), "abc".to_string()]
        );
    }

    /// Continuing a chat keeps its id, so the run is resumed rather than
    /// assigned one, and no fork is made: a fork would be a new id and a new
    /// transcript that no other client is watching. tech.md 6.5.
    #[test]
    fn continuing_a_chat_keeps_its_own_id_and_never_forks() {
        let args = spawn_args("chat-id", true, None, None, None, None, None);
        assert_eq!(args, vec!["--resume".to_string(), "chat-id".to_string()]);
        assert!(!args.iter().any(|a| a == "--fork-session"));
        // Never both: the CLI refuses the pair unless it forks.
        assert!(!args.iter().any(|a| a == "--session-id"));
    }

    /// The one combination the CLI takes all three flags in: measured live on
    /// 2.1.263, a fork with an assigned id answers questions about the old
    /// conversation and writes only to the new transcript. Without
    /// `--session-id` the CLI picks the new id and Peekle loses the chat it
    /// just made. tech.md 6.5.
    #[test]
    fn a_fork_names_the_chat_it_copies_and_the_id_it_becomes() {
        let args = spawn_args(
            "new-id",
            false,
            Some("old-id"),
            Some("hi"),
            None,
            None,
            None,
        );

        assert_eq!(
            args,
            vec![
                "--resume",
                "old-id",
                "--fork-session",
                "--session-id",
                "new-id",
                "hi"
            ]
        );
    }

    /// A fork is never in place: the chat it copies and the chat it becomes
    /// are two different ids, and running both rules would put the copy back
    /// on top of the original.
    #[test]
    fn forking_beats_resuming_in_place_when_both_are_asked_for() {
        let args = spawn_args("new-id", true, Some("old-id"), None, None, None, None);

        assert_eq!(args.first().map(String::as_str), Some("--resume"));
        assert_eq!(args.get(1).map(String::as_str), Some("old-id"));
        assert!(args.iter().any(|arg| arg == "--fork-session"));
        assert_eq!(args.iter().filter(|arg| *arg == "--resume").count(), 1);
    }

    /// The first message of a fork rides as an argument, because a TUI that is
    /// still starting swallows anything written into it. tech.md 6.5.
    #[test]
    fn the_first_message_is_handed_over_rather_than_typed() {
        let args = spawn_args(
            "chat-id",
            true,
            None,
            Some("what did I say?"),
            None,
            None,
            None,
        );
        assert_eq!(args.last().map(String::as_str), Some("what did I say?"));
        // Positional: it carries no flag of its own and cannot be read as one.
        assert!(!args.iter().any(|a| a == "--prompt" || a == "-p"));

        // Nothing to say means nothing appended, not an empty argument.
        for empty in [Some(""), Some("   "), None] {
            let args = spawn_args("new-id", false, None, empty, None, None, None);
            assert_eq!(args, vec!["--session-id".to_string(), "new-id".to_string()]);
        }
    }

    /// A pick made before the session ran travels as the CLI's own flags,
    /// and ahead of the prompt: the prompt is the first turn, and a `/model`
    /// line typed behind it would only reach the second. tech.md 6.15.
    #[test]
    fn settings_picked_before_the_first_turn_ride_as_flags_ahead_of_it() {
        let args = spawn_args(
            "new-id",
            false,
            None,
            Some("hi"),
            Some("opus"),
            Some("high"),
            Some("plan"),
        );
        let expected: Vec<String> = [
            "--session-id",
            "new-id",
            "--model",
            "opus",
            "--effort",
            "high",
            // The permission mode has no line to be typed as, so the flag is
            // the whole of it. tech.md 6.19.
            "--permission-mode",
            "plan",
            "hi",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(args, expected);

        // Nothing picked means no flag, not an empty one.
        let args = spawn_args("new-id", false, None, Some("hi"), Some(" "), None, None);
        assert_eq!(args, vec!["--session-id", "new-id", "hi"]);
    }

    /// The nudge is one newline and nothing else. Anything more would be text
    /// written twice, and a turn started twice is worse than a turn not
    /// started at all. tech.md 6.5.
    #[test]
    fn the_nudge_adds_nothing_to_the_box_it_submits() {
        assert_eq!(NUDGE, b"\r");
        assert_eq!(NUDGE, message_writes("anything").last().unwrap().as_slice());
        assert_ne!(NUDGE, INTERRUPT);
    }

    #[test]
    fn newline_is_its_own_write() {
        let writes = message_writes("build it");
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], b"build it");
        assert_eq!(writes[1], b"\r");
    }

    /// A slash command answers the question it raises.
    ///
    /// `/effort ultracode` on a cached conversation draws `Change effort
    /// level?` and the TUI stays modal on it. The message typed next went
    /// into that dialog and its newline answered the question: the words were
    /// gone, the agent never saw them, and the island had already drawn the
    /// bubble. Measured on a live 2.1.263 with Opus 5. tech.md 6.15.
    #[test]
    fn a_slash_command_answers_its_own_question() {
        let writes = command_writes("/effort ultracode");

        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0], b"/effort ultracode");
        assert_eq!(writes[1], b"\r");
        assert_eq!(writes[2], NUDGE, "and the answer is one plain newline");
    }

    /// What takes the answering newline and what does not. A path is a thing
    /// people send agents, and it starts with a slash.
    #[test]
    fn a_slash_command_is_told_from_a_message_that_starts_with_one() {
        for line in ["/compact", "/model opus", "/effort ultracode", "/status"] {
            assert!(is_slash_command(line), "{line}");
        }
        for text in [
            "/Users/dev/peekle/src/lib.rs",
            "/tmp/shot.png look at this",
            "read /etc/hosts",
            "/model opus
and then stop",
            "",
            "/",
        ] {
            assert!(!is_slash_command(text), "{text:?}");
        }
    }

    /// It is a message plus one newline, and nothing else: a dialog that is
    /// not there must be answered by something an empty input box ignores.
    #[test]
    fn the_answer_adds_nothing_a_box_could_keep() {
        let line = "/model opus";
        let message = message_writes(line);
        let command = command_writes(line);

        assert_eq!(command[..2], message[..]);
        assert_eq!(command[2], b"\r");
    }

    /// Long enough for the TUI to run the command and draw what it asks.
    #[test]
    fn the_answer_waits_for_the_question_to_be_drawn() {
        assert!(CONFIRM_GAP >= std::time::Duration::from_millis(300));
        assert!(CONFIRM_GAP <= std::time::Duration::from_millis(1000));
    }

    /// Below the measured threshold the two writes arrive as one read, the TUI
    /// takes them for a paste, and a long reply never leaves the field.
    #[test]
    fn newline_waits_out_the_paste_window() {
        assert!(ENTER_GAP >= std::time::Duration::from_millis(50));
        assert!(ENTER_GAP <= std::time::Duration::from_millis(250));
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

    /// One session's half-second write must not hold up another's, least of
    /// all an interrupt: the map is released before anything goes out, and
    /// only that session's own writer is held. tech.md 6.5.
    #[test]
    fn a_write_into_one_session_does_not_hold_the_others() {
        let host = std::sync::Arc::new(PtyHost::new());

        // Nothing is spawned here, so every call refuses -- and that refusal
        // is the map lookup itself. If the map were held across the gaps of a
        // command, this would be the call that blocked on it.
        let busy = std::sync::Arc::clone(&host);
        let writer = std::thread::spawn(move || busy.command("a", "/effort ultracode"));

        let started = std::time::Instant::now();
        assert!(matches!(host.interrupt("b"), Err(PtyError::NotOwned)));
        assert!(
            started.elapsed() < CONFIRM_GAP,
            "an interrupt waited on another session's write"
        );
        assert!(matches!(writer.join().unwrap(), Err(PtyError::NotOwned)));
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

    /// Observed live: Peekle launched from inside a Claude Code session passed
    /// `CLAUDE_CODE_SESSION_ID` and `CLAUDE_CODE_CHILD_SESSION` down to the
    /// agent it started. The first argues with the `--session-id` we assign,
    /// so hooks arrived under an id matching no card and the reply sat
    /// unconfirmed; the second switched transcript saving off.
    #[test]
    fn the_markers_of_a_parent_session_are_not_passed_down() {
        for key in [
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_MESSAGING_SOCKET",
        ] {
            assert!(is_session_marker(key), "{key} has to be scrubbed");
        }
    }

    /// Scrubbing must not reach past Claude Code's own markers: the session
    /// still needs the environment the user works in.
    #[test]
    fn the_rest_of_the_environment_survives() {
        for key in ["PATH", "TERM", "HOME", "SHELL", "LANG", "CLAUDECODE"] {
            assert!(!is_session_marker(key), "{key} must be left alone");
        }
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
            resume: false,
            fork_from: None,
            prompt: None,
            model: None,
            effort: None,
            mode: None,
            thinking: None,
        };
        let result = host.spawn(Path::new("/bin/echo"), &spec, |_| {}, |_| {});
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
            resume: false,
            fork_from: None,
            prompt: None,
            model: None,
            effort: None,
            mode: None,
            thinking: None,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        host.spawn(
            Path::new("/bin/echo"),
            &spec,
            move |id| {
                let _ = tx.send(id);
            },
            |_| {},
        )
        .expect("spawn echo");

        assert!(host.owns(&spec.session_id));
        let exited = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("exit reported");
        assert_eq!(exited, spec.session_id);
    }
}
