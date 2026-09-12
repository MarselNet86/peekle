//! Signing in, through Claude Code's own command. tech.md 6.16.
//!
//! Peekle does not have an OAuth client and must not grow one. The Keychain
//! entry belongs to Claude Code -- Peekle only ever reads it (section 11) --
//! and minting tokens against someone else's `client_id` would be passing
//! Peekle off as Claude Code. So the sign-in here is `claude auth login`,
//! driven in a pty, and the credential it writes stays Claude Code's.
//!
//! Same split as [`crate::pty`]: everything that touches a real process is
//! behind [`SignInHost`], and the parts that decide what the output means are
//! free functions, so they can be tested against captured bytes without a
//! `claude` binary on the machine.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[cfg(not(target_os = "windows"))]
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

use crate::types::{SignInFix, SignInNeed, SignInStage, SignInState};

/// Sign in against a Claude subscription, which is the account whose token the
/// usage endpoint answers for. `--console` would be an API-billed account and
/// a different thing entirely. tech.md 6.16.
pub const LOGIN_ARGS: &[&str] = &["auth", "login", "--claudeai"];

/// Ask the CLI whether it considers itself signed in. Local: no network, no
/// Keychain dialog, which is what makes it safe to call before offering a
/// sign-in that might not be the thing that helps. tech.md 6.16.
pub const STATUS_ARGS: &[&str] = &["auth", "status", "--json"];

/// What the CLI is asked to print so the two above can be trusted.
pub const HELP_ARGS: &[&str] = &["--help"];

/// Whether this Claude Code still carries the `auth` command the panel drives.
///
/// Read off the CLI's own `Commands:` block rather than compared as a version
/// number. The command list is what actually decides -- a build without `auth`
/// answers `claude auth login` by treating the words as a prompt and opening
/// an interactive session, which is a login that hangs on `Starting` forever
/// with no address and no exit. A version comparison would have to be edited
/// every time the CLI moves; this reads the answer from the CLI in front of
/// us. tech.md 6.16 and 6.27.
pub fn supports_login(help: &str) -> bool {
    let plain = strip_escapes(help);
    let mut in_commands = false;
    for line in plain.lines() {
        let indented = line.starts_with(char::is_whitespace);
        if !indented {
            // `Commands:` opens the block and the next unindented line closes
            // it, so a command named like a section heading cannot be read out
            // of the wrong list.
            in_commands = line.trim_end().eq_ignore_ascii_case("commands:");
            continue;
        }
        if in_commands && line.split_whitespace().next() == Some("auth") {
            return true;
        }
    }
    false
}

/// The command that puts Claude Code on this machine, in the shell that runs
/// it. Anthropic's own installer, and nothing bundled by Peekle: the CLI is
/// theirs to ship. tech.md 6.16 and 6.27.
pub fn install_fix() -> SignInFix {
    #[cfg(target_os = "windows")]
    {
        SignInFix {
            need: SignInNeed::Install,
            shell: "PowerShell".to_string(),
            command: "irm https://claude.ai/install.ps1 | iex".to_string(),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        SignInFix {
            need: SignInNeed::Install,
            shell: "Terminal".to_string(),
            command: "curl -fsSL https://claude.ai/install.sh | bash".to_string(),
        }
    }
}

/// The command that brings an installed Claude Code up to a version that can
/// sign in. One line on every platform; only the shell it is typed in differs.
pub fn update_fix() -> SignInFix {
    SignInFix {
        need: SignInNeed::Update,
        shell: install_fix().shell,
        command: "claude update".to_string(),
    }
}

/// What the CLI prints when it wants the code from the authorize page.
///
/// Matched on the stable half of the line. The whole prompt is
/// `Paste code here if prompted >`, and the tail is the part most likely to be
/// reworded.
const CODE_PROMPT: &str = "Paste code here";

/// Everything the terminal draws that is not text.
///
/// The URL arrives wrapped in an OSC 8 hyperlink, which puts the same address
/// in the stream twice: once as the link parameter and once as the visible
/// label. Dropping the escape sequences leaves the visible copy alone, so the
/// address is found once rather than twice. Captured from a live run, see
/// tech.md 6.16.
pub fn strip_escapes(chunk: &str) -> String {
    let mut out = String::with_capacity(chunk.len());
    let mut chars = chunk.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            // OSC: runs to a BEL, or to the ST pair `ESC \`.
            Some(']') => {
                while let Some(inner) = chars.next() {
                    if inner == '\u{7}' {
                        break;
                    }
                    if inner == '\u{1b}' {
                        chars.next_if_eq(&'\\');
                        break;
                    }
                }
            }
            // CSI: parameter and intermediate bytes, then one final byte.
            Some('[') => {
                for inner in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&inner) {
                        break;
                    }
                }
            }
            // Anything else is a two-byte escape; both bytes go.
            Some(_) => {}
            None => {}
        }
    }
    out
}

/// The authorize address in a chunk of the CLI's output, if it carries one.
///
/// Any `https://` run is taken rather than a known host: the address is
/// Claude Code's to change, and a hardcoded host would fail silently the day
/// it moves, leaving the panel with no way out at all. What makes it the
/// authorize URL is that this process printed it.
pub fn authorize_url(chunk: &str) -> Option<String> {
    let plain = strip_escapes(chunk);
    let start = plain.find("https://")?;
    let rest = &plain[start..];
    let end = rest
        .find(|ch: char| ch.is_whitespace() || ch == '\u{7}')
        .unwrap_or(rest.len());
    let url = rest[..end].trim_end_matches(['.', ',', ')']);
    (url.len() > "https://".len()).then(|| url.to_string())
}

/// Whether the CLI is standing at the paste prompt.
pub fn wants_code(chunk: &str) -> bool {
    strip_escapes(chunk).contains(CODE_PROMPT)
}

/// What `claude auth status --json` says about being signed in.
///
/// `None` for anything that is not an object with that field: a CLI too old
/// for the command, a version that renamed it, or output that is not JSON at
/// all. `None` means "do not know", and the caller must not read it as "no" --
/// telling someone they are signed out on the strength of a parse failure is
/// how a working account gets sent through a pointless login. tech.md 6.16.
pub fn logged_in(raw: &str) -> Option<bool> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()?
        .get("loggedIn")?
        .as_bool()
}

/// The bytes one pasted code puts on the wire.
///
/// Same shape and the same reason as [`crate::pty::message_writes`]: the
/// newline is a write of its own, because a line and its return inside one
/// write arrive as a paste rather than as a submission.
pub fn code_writes(code: &str) -> Vec<Vec<u8>> {
    vec![code.trim().as_bytes().to_vec(), b"\r".to_vec()]
}

#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    #[error("no claude binary found")]
    NoBinary,
    #[error("sign-in is not running")]
    NotRunning,
    #[error("the code is empty")]
    EmptyCode,
    #[error("sign-in failed: {0}")]
    Pty(String),
}

/// The one sign-in process, and the handles that keep it alive.
struct Running {
    writer: Box<dyn Write + Send>,
    handles: Handles,
}

/// What the process was started through, and so what has to be put down.
///
/// A pty everywhere the CLI behaves in one. On Windows it does not: under a
/// pseudoconsole `claude auth login` sits at zero CPU, prints nothing and
/// never exits -- the first live sign-in there stood on `Opening your
/// browser` for good. Started on plain pipes the same binary prints the
/// address, opens the browser and reads the code, so that is how it is
/// started there. The parts of 6.16 that read the output do not know which:
/// pipes carry no escapes to strip, and stripping none is not an error.
/// tech.md 6.16 and 6.27.
enum Handles {
    #[cfg(not(target_os = "windows"))]
    Pty {
        child: Box<dyn portable_pty::Child + Send + Sync>,
        _master: Box<dyn portable_pty::MasterPty + Send>,
    },
    #[cfg(target_os = "windows")]
    Pipes { child: std::process::Child },
}

impl Handles {
    fn kill(&mut self) {
        match self {
            #[cfg(not(target_os = "windows"))]
            Handles::Pty { child, .. } => {
                let _ = child.kill();
            }
            #[cfg(target_os = "windows")]
            Handles::Pipes { child } => {
                let _ = child.kill();
            }
        }
    }
}

/// A process just started: what it writes, what it reads, what keeps it.
struct Spawned {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    handles: Handles,
}

/// Starts the login in the pty the CLI opens its browser from. tech.md 6.16.
#[cfg(not(target_os = "windows"))]
fn spawn(binary: &Path) -> Result<Spawned, SignInError> {
    let system = NativePtySystem::default();
    let pair = system
        .openpty(PtySize {
            // Wide enough that the authorize address is not wrapped: a
            // line break inside it would split it in the stream and leave
            // the panel offering half an address.
            rows: 40,
            cols: 400,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| SignInError::Pty(err.to_string()))?;

    let mut command = CommandBuilder::new(binary);
    for arg in LOGIN_ARGS {
        command.arg(arg);
    }
    command.env("TERM", "xterm-256color");
    command.env("PATH", crate::pty::session_path());

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|err| SignInError::Pty(err.to_string()))?;
    drop(pair.slave);

    let writer = pair
        .master
        .take_writer()
        .map_err(|err| SignInError::Pty(err.to_string()))?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| SignInError::Pty(err.to_string()))?;

    Ok(Spawned {
        reader,
        writer,
        handles: Handles::Pty {
            child,
            _master: pair.master,
        },
    })
}

/// Starts the login on pipes, with no console of its own: a console child of
/// a windowed parent gets a fresh console window otherwise, and one flashed
/// up on every press. tech.md 6.27.
#[cfg(target_os = "windows")]
fn spawn(binary: &Path) -> Result<Spawned, SignInError> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut child = Command::new(binary)
        .args(LOGIN_ARGS)
        .env("PATH", crate::pty::session_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|err| SignInError::Pty(err.to_string()))?;
    let reader = child
        .stdout
        .take()
        .ok_or_else(|| SignInError::Pty("no stdout".to_string()))?;
    let writer = child
        .stdin
        .take()
        .ok_or_else(|| SignInError::Pty("no stdin".to_string()))?;

    Ok(Spawned {
        reader: Box::new(reader),
        writer: Box::new(writer),
        handles: Handles::Pipes { child },
    })
}

/// Puts a run down, off whatever thread asked for it.
///
/// Dropping the master closes the pseudoconsole, and on Windows
/// `ClosePseudoConsole` waits: for the console host to go, which waits for
/// every process still attached to it, which is not always the one that was
/// just killed. The second live sign-in on Windows ended in a window that
/// stopped answering, because the thread that pressed Cancel was the main
/// thread and the drop sat on it holding the `running` lock. So the run is
/// taken out under the lock, the lock is let go, and the kill and the drop
/// happen on a thread nobody waits for. A host that never lets go costs one
/// parked thread, and that is the whole price. tech.md 6.16 and 6.27.
fn put_down(running: Option<Running>) {
    let Some(mut running) = running else {
        return;
    };
    std::thread::spawn(move || {
        running.handles.kill();
        drop(running);
        tracing::debug!("the sign-in process is put down");
    });
}

/// How much of the output is carried between reads, so an address split across
/// two of them is still found whole.
const CARRY: usize = 4096;

/// The sign-in, if one is running. tech.md 6.16.
///
/// One at a time: a second `claude auth login` would race the first for the
/// same Keychain entry, and there is only one panel to show either of them in.
#[derive(Default)]
pub struct SignInHost {
    running: Mutex<Option<Running>>,
    state: Mutex<SignInState>,
    /// Bumped by every start and every cancel. The reader thread carries the
    /// number it was born with and reports nothing once it no longer matches,
    /// so a run that was replaced or cancelled cannot write state for the one
    /// that took its place -- and a process dying from its own `kill` cannot
    /// reopen a panel the user just closed. Rule 10.
    generation: Mutex<u64>,
}

impl SignInHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> SignInState {
        self.lock_state().clone()
    }

    /// Starts `claude auth login` in a pty and reports every step through
    /// `on_state`.
    ///
    /// Claude Code opens the browser itself; nothing here does. The output is
    /// read, unlike an owned session's, because the authorize address and the
    /// paste prompt exist nowhere else. tech.md 6.16.
    pub fn start<F>(
        self: &Arc<Self>,
        binary: &Path,
        on_state: F,
    ) -> Result<SignInState, SignInError>
    where
        F: Fn(SignInState) + Send + 'static,
    {
        // A press while one is already up replaces it. Two of them would be
        // two processes fighting over one Keychain entry.
        self.cancel();
        let generation = self.bump();

        let Spawned {
            mut reader,
            writer,
            handles,
        } = spawn(binary)?;

        let starting = SignInState {
            stage: SignInStage::Starting,
            url: None,
            needs_code: false,
            error: None,
            fix: None,
        };
        *self.lock_state() = starting.clone();
        self.lock_running().replace(Running { writer, handles });

        let host = Arc::clone(self);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            // An address can straddle two reads. Only the tail is kept, and
            // only as much as one could need, so a long login cannot grow this
            // without bound.
            let mut tail = String::new();
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                tail.push_str(&String::from_utf8_lossy(&buffer[..read]));
                host.saw(&tail, generation, &on_state);
                trim_to_carry(&mut tail);
            }
            host.exited(generation, &on_state);
        });

        Ok(starting)
    }

    /// Puts the code from the authorize page into the process's stdin.
    ///
    /// The code is a credential of the same class as the token: its length may
    /// be logged, never its value. tech.md 6.16 and rule 11.
    pub fn submit_code(&self, code: &str) -> Result<SignInState, SignInError> {
        if code.trim().is_empty() {
            return Err(SignInError::EmptyCode);
        }
        {
            let mut running = self.lock_running();
            let running = running.as_mut().ok_or(SignInError::NotRunning)?;

            tracing::debug!(code_len = code.trim().len(), "submitting the sign-in code");
            for (index, chunk) in code_writes(code).into_iter().enumerate() {
                // Separate in time as well as in bytes, for the reason in
                // `crate::pty::ENTER_GAP`: glued together they arrive as a
                // paste rather than as a submission.
                if index > 0 {
                    std::thread::sleep(crate::pty::ENTER_GAP);
                }
                running
                    .writer
                    .write_all(&chunk)
                    .map_err(|err| SignInError::Pty(err.to_string()))?;
                running
                    .writer
                    .flush()
                    .map_err(|err| SignInError::Pty(err.to_string()))?;
            }
        }

        let mut state = self.lock_state();
        state.stage = SignInStage::Finishing;
        state.needs_code = false;
        Ok(state.clone())
    }

    /// Ends whatever is running and puts the panel away.
    ///
    /// A no-op when nothing is running: a process that exited a moment earlier
    /// is not an error. The generation moves either way, so the reader thread
    /// of the run being killed reports nothing after this. tech.md 6.16.
    pub fn cancel(&self) {
        self.bump();
        let running = self.lock_running().take();
        if running.is_some() {
            tracing::debug!("the sign-in was cancelled");
        }
        put_down(running);
        *self.lock_state() = SignInState::idle();
    }

    /// Settles a run that the caller has decided about: signed in, or not.
    ///
    /// The pty closing says the process is gone, not whether the account is
    /// signed in, and an exit code read through a terminal is a poor thing to
    /// believe. So the last word belongs to whoever asked the CLI. Resolved
    /// exactly once per run, like every other blocking path. Rule 10.
    pub fn settle(&self, done: bool, error: Option<String>) -> SignInState {
        self.lock_running().take();
        let mut state = self.lock_state();
        state.stage = if done {
            SignInStage::Done
        } else {
            SignInStage::Failed
        };
        state.needs_code = false;
        state.error = error;
        state.clone()
    }

    /// What the output means so far, reported only when it changed something.
    fn saw<F: Fn(SignInState)>(&self, tail: &str, generation: u64, on_state: &F) {
        if !self.current(generation) {
            return;
        }
        let mut state = self.lock_state();
        let before = state.clone();

        if state.url.is_none() {
            if let Some(url) = authorize_url(tail) {
                // The length, never the address: it carries `code_challenge`
                // and `state`. tech.md 6.16 and rule 11.
                tracing::debug!(url_len = url.len(), "the authorize address is out");
                state.url = Some(url);
                state.stage = SignInStage::Waiting;
            }
        }
        if !state.needs_code && wants_code(tail) {
            state.needs_code = true;
            state.stage = SignInStage::Waiting;
        }

        if *state != before {
            let next = state.clone();
            drop(state);
            on_state(next);
        }
    }

    /// The process is gone. Whether that was a success is not decided here.
    fn exited<F: Fn(SignInState)>(&self, generation: u64, on_state: &F) {
        if !self.current(generation) {
            return;
        }
        // The process is gone, so nothing below needs its handles; a run left
        // in the slot would be the next start's to tear down, on its thread.
        put_down(self.lock_running().take());
        let next = {
            let mut state = self.lock_state();
            if matches!(state.stage, SignInStage::Done | SignInStage::Failed) {
                return;
            }
            state.stage = SignInStage::Finishing;
            state.clone()
        };
        tracing::debug!("the sign-in process exited");
        on_state(next);
    }

    /// Whether this run is still the one the host is describing.
    fn current(&self, generation: u64) -> bool {
        *self.lock_generation() == generation
    }

    fn bump(&self) -> u64 {
        let mut generation = self.lock_generation();
        *generation += 1;
        *generation
    }

    fn lock_running(&self) -> std::sync::MutexGuard<'_, Option<Running>> {
        self.running
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, SignInState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_generation(&self) -> std::sync::MutexGuard<'_, u64> {
        self.generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Keeps the carried tail bounded, cutting on a character boundary so the
/// lossy decoding above is never sliced through a multi-byte character.
fn trim_to_carry(tail: &mut String) {
    if tail.len() <= CARRY {
        return;
    }
    let want = tail.len() - CARRY;
    let cut = (0..=want)
        .rev()
        .find(|at| tail.is_char_boundary(*at))
        .unwrap_or(0);
    tail.drain(..cut);
}

/// Shared handle, because the commands and the reader thread both hold it.
pub type SharedSignInHost = Arc<SignInHost>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from a live `claude auth login --claudeai` in a pty, with the
    /// escapes as they came. The address appears twice -- once as the OSC 8
    /// link parameter, once as its visible label -- which is exactly the
    /// hazard this parses around. tech.md 6.16.
    const LIVE: &str = "\u{1b}]8;;https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a&state=pFKX\u{7}https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a&state=pFKX\u{1b}]8;;\u{7}\r\nPaste code here if prompted > ";

    #[test]
    fn takes_the_address_out_of_the_hyperlink_once() {
        let url = authorize_url(LIVE).expect("the live output carries an address");
        assert!(url.starts_with("https://claude.com/cai/oauth/authorize?"));
        assert!(
            !url.contains("https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a&state=pFKXhttps://"),
            "the link parameter and its label must not be glued into one address"
        );
        assert!(!url.contains('\u{7}'), "no terminal bytes ride along");
    }

    #[test]
    fn knows_the_paste_prompt_when_it_comes() {
        assert!(wants_code(LIVE));
        assert!(!wants_code("Opening browser to sign in"));
    }

    /// Captured from `claude --help` on a build that still has the command
    /// this panel drives.
    const HELP_WITH_AUTH: &str = concat!(
        "Usage: claude [options] [command] [prompt]\n\n",
        "Options:\n",
        "  -h, --help   Display help for command\n\n",
        "Commands:\n",
        "  auth         Manage authentication\n",
        "  doctor       Check the health of your Claude Code auto-updater\n",
        "  update       Check for updates and install if available\n",
    );

    /// Captured from `claude --help` on 2.1.31, which has no `auth` at all:
    /// `claude auth login` there is a prompt, not a command, and the sign-in
    /// hangs on `Starting` forever. tech.md 6.16.
    const HELP_WITHOUT_AUTH: &str = concat!(
        "Usage: claude [options] [command] [prompt]\n\n",
        "Commands:\n",
        "  doctor       Check the health of your Claude Code auto-updater\n",
        "  install      Install Claude Code native build\n",
        "  mcp          Configure and manage MCP servers\n",
        "  setup-token  Set up a long-lived authentication token\n",
        "  update       Check for updates and install if available\n",
    );

    #[test]
    fn the_command_list_decides_whether_a_login_can_be_driven() {
        assert!(supports_login(HELP_WITH_AUTH));
        assert!(!supports_login(HELP_WITHOUT_AUTH));
    }

    /// A word that only looks like the command does not count: `auth` has to
    /// be a command in the command list, not a mention in a description or an
    /// option in the block above it.
    #[test]
    fn only_a_command_named_auth_counts() {
        assert!(!supports_login(""));
        assert!(!supports_login("  auth  Manage authentication\n"));
        assert!(!supports_login(
            "Options:\n  --auth <mode>  something\n\nCommands:\n  update  x\n"
        ));
        assert!(!supports_login(
            "Commands:\n  doctor  x\n\nExamples:\n  auth login\n"
        ));
        assert!(!supports_login(
            "Commands:\n  authorize  not the same command\n"
        ));
    }

    /// The CLI paints its help, and a colour code in front of the name would
    /// hide the command from a plain read of the first word.
    #[test]
    fn colours_in_the_help_do_not_hide_the_command() {
        let painted = "Commands:\n  \u{1b}[1mauth\u{1b}[0m  Manage authentication\n";
        assert!(supports_login(painted));
    }

    /// Each platform is told to use its own shell, and neither command is
    /// empty: a panel that hands over a blank line is worse than one that
    /// hands over nothing. tech.md 6.16.
    #[test]
    fn every_platform_gets_a_command_and_a_shell_for_it() {
        for fix in [install_fix(), update_fix()] {
            assert!(!fix.shell.is_empty());
            assert!(!fix.command.is_empty());
        }
        assert_eq!(update_fix().command, "claude update");
        assert_eq!(install_fix().shell, update_fix().shell);
        assert_eq!(install_fix().need, SignInNeed::Install);
        assert_eq!(update_fix().need, SignInNeed::Update);
    }

    /// Ordinary output is neither an address nor a prompt. Reading a stray
    /// line as either would put the panel into a state the CLI is not in.
    #[test]
    fn ordinary_output_says_nothing() {
        assert_eq!(authorize_url("Opening browser to sign in"), None);
        assert!(!wants_code("Opening browser to sign in"));
        assert_eq!(authorize_url(""), None);
    }

    /// An address split across two reads is still one address once the second
    /// read arrives, which is why the reader carries a tail.
    #[test]
    fn finds_an_address_that_arrived_in_two_pieces() {
        let (head, rest) = LIVE.split_at(60);
        assert_eq!(authorize_url(head), None, "half an address is not one");
        let whole = format!("{head}{rest}");
        assert!(authorize_url(&whole).is_some());
    }

    #[test]
    fn reads_what_the_cli_says_about_being_signed_in() {
        assert_eq!(
            logged_in(r#"{"loggedIn":true,"authMethod":"claude.ai"}"#),
            Some(true)
        );
        assert_eq!(logged_in(r#"{"loggedIn":false}"#), Some(false));
    }

    /// Not knowing is not the same as being signed out: a CLI too old for the
    /// command, or output that is not JSON, must not send a working account
    /// through a login it does not need. tech.md 6.16.
    #[test]
    fn output_it_cannot_read_is_not_a_no() {
        assert_eq!(logged_in("command not found"), None);
        assert_eq!(logged_in("{}"), None);
        assert_eq!(logged_in(r#"{"loggedIn":"yes"}"#), None);
        assert_eq!(logged_in(""), None);
    }

    #[test]
    fn the_code_and_its_return_are_separate_writes() {
        assert_eq!(
            code_writes("  abc123 "),
            vec![b"abc123".to_vec(), b"\r".to_vec()]
        );
    }
}
