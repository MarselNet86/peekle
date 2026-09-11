//! The live-session registry Claude Code keeps, and the one decision made
//! from it: where a reply to an observed chat goes. tech.md 6.5 and R-17.
//!
//! Every `claude` process (2.1.26x and up) writes `~/.claude/sessions/<pid>.json`
//! on start and removes it on exit. That record is the only fact about "who
//! holds this chat" that is not a guess: it names the process, and the
//! process either answers `kill(pid, 0)` or it does not. The transcript's
//! mtime, which stood here before (v34), knew only that the file had been
//! written recently, and a chat that had just answered looked busy for
//! another ninety seconds.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The one version of the peer protocol this code speaks. Anything else
/// falls to `Route::Busy`, never to a resume. R-17.
pub const PEER_PROTOCOL: u64 = 1;

/// A `claude` process as its registry record describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveSession {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    /// `procStart`, the process start time as `ps -o lstart=` prints it. A
    /// pid is reused; this string is not, so the two together name a process.
    pub proc_start: Option<String>,
    pub version: String,
    /// `cli`, `claude-vscode`, `claude-desktop`.
    pub entrypoint: Option<String>,
    pub peer_protocol: Option<u64>,
    /// `messagingSocketPath`: the unix socket the process takes messages on.
    pub inbox: Option<PathBuf>,
}

impl LiveSession {
    /// Whether this process publishes an inbox this code can speak to.
    pub fn takes_messages(&self) -> bool {
        self.inbox.is_some() && self.peer_protocol == Some(PEER_PROTOCOL)
    }
}

/// The shape on disk. Unknown fields are ignored: Claude Code adds fields
/// between versions, and the record is theirs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    pid: u32,
    session_id: String,
    #[serde(default)]
    cwd: String,
    proc_start: Option<String>,
    #[serde(default)]
    version: String,
    entrypoint: Option<String>,
    peer_protocol: Option<u64>,
    messaging_socket_path: Option<String>,
}

/// Where Claude Code keeps the records, under the user's home.
pub fn default_root() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| Path::new(&home).join(".claude").join("sessions"))
}

/// One record, or nothing for a file that is not one. A registry directory
/// holds keys and half-written temp files next to the records, and none of
/// them is a reason to fail.
pub fn parse_record(text: &str) -> Option<LiveSession> {
    let record: Record = serde_json::from_str(text).ok()?;
    if record.session_id.is_empty() {
        return None;
    }
    Some(LiveSession {
        pid: record.pid,
        session_id: record.session_id,
        cwd: record.cwd,
        proc_start: record.proc_start,
        version: record.version,
        entrypoint: record.entrypoint,
        peer_protocol: record.peer_protocol,
        inbox: record
            .messaging_socket_path
            .filter(|path| !path.is_empty())
            .map(PathBuf::from),
    })
}

/// Every `<pid>.json` under `root` that parses. Order is the directory's.
pub fn records(root: &Path) -> Vec<LiveSession> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_record_name(path))
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|text| parse_record(&text))
        .collect()
}

fn is_record_name(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "json")
        && path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit()))
}

/// The process holding `session_id`, judged by `alive`, if any.
///
/// Two records can name one chat: a crash leaves a stale record behind, and a
/// resumed chat has the resumer's record next to the original's. The live
/// ones count, and among those the one with an inbox wins, because it is the
/// one that can take the words.
pub fn find(
    root: &Path,
    session_id: &str,
    alive: &dyn Fn(&LiveSession) -> bool,
) -> Option<LiveSession> {
    let live: Vec<LiveSession> = records(root)
        .into_iter()
        .filter(|record| record.session_id == session_id)
        .filter(|record| alive(record))
        .collect();
    live.iter()
        .find(|record| record.takes_messages())
        .or_else(|| live.first())
        .cloned()
}

/// `find` judged by the real process table.
pub fn find_live(root: &Path, session_id: &str) -> Option<LiveSession> {
    find(root, session_id, &process_alive)
}

/// Whether the process a record names is still the process it named.
///
/// `kill(pid, 0)` says a process with that pid exists; `procStart` says it is
/// the same one, because a pid is reused and a start time is not. Doubt goes
/// one way: when `ps` cannot be asked, the process is alive. The cost of
/// being wrong in that direction is a wait; in the other, two agents on one
/// transcript. tech.md 6.5.
///
/// The record keeps its clock in UTC and `ps` prints the local one, so the
/// start is read in both and either match is the same process. Seen live
/// 2026-09-09: a process VS Code held read as dead for five hours' worth of
/// difference, and the island resumed its chat a second time. tech.md 6.5.
pub fn process_alive(live: &LiveSession) -> bool {
    if !pid_exists(live.pid) {
        return false;
    }
    let Some(expected) = live.proc_start.as_deref() else {
        return true;
    };
    let starts = proc_starts_of(live.pid);
    if starts.is_empty() {
        return true;
    }
    starts.iter().any(|actual| same_start(actual, expected))
}

#[cfg(unix)]
fn pid_exists(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: `kill` with signal 0 sends nothing; it only asks whether the
    // pid is deliverable to. EPERM means it exists and is someone else's,
    // which for this question is still "exists".
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Windows: a handle to the process, or nothing. `OpenProcess` with the
/// least it can ask for, so a process of another user still answers "exists"
/// with an access error rather than "gone". tech.md 6.27.
#[cfg(windows)]
fn pid_exists(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY: a query handle that is closed on the spot; no memory is shared.
    match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(handle) => {
            // SAFETY: the handle came from the call above and is used once.
            let _ = unsafe { CloseHandle(handle) };
            true
        }
        Err(err) => err.code() == ERROR_ACCESS_DENIED.to_hresult(),
    }
}

/// The start of `pid` as `ps` prints it, once under `TZ=UTC` and once in the
/// local clock. Empty when `ps` could not be asked at all -- which on Windows
/// is always, so an existing pid there counts as alive. tech.md 6.27, R-22.
#[cfg(not(unix))]
fn proc_starts_of(_pid: u32) -> Vec<String> {
    Vec::new()
}

/// The start of `pid` as `ps` prints it, once under `TZ=UTC` and once in the
/// local clock. Empty when `ps` could not be asked at all.
#[cfg(unix)]
fn proc_starts_of(pid: u32) -> Vec<String> {
    [Some("UTC"), None]
        .into_iter()
        .filter_map(|zone| {
            let mut command = std::process::Command::new("ps");
            command.args(["-o", "lstart=", "-p", &pid.to_string()]);
            if let Some(zone) = zone {
                command.env("TZ", zone);
            }
            let output = command.output().ok()?;
            let start = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!start.is_empty()).then_some(start)
        })
        .collect()
}

/// `ps` under `TZ=UTC`, which is the clock the registry record keeps. Public
/// so a test can name the process it runs in the way Claude Code would.
#[cfg(unix)]
pub fn utc_start_of(pid: u32) -> Option<String> {
    let output = std::process::Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .env("TZ", "UTC")
        .output()
        .ok()?;
    let start = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!start.is_empty()).then_some(start)
}

/// `ps` pads the day of the month with a space; so does Claude Code, but the
/// comparison should not hang on whitespace either way.
pub fn same_start(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    norm(a) == norm(b)
}

/// Where a reply to an observed chat goes. tech.md 6.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// A live process publishes an inbox: the words go to it, and the chat
    /// stays its own.
    Inbox(LiveSession),
    /// Nobody holds the chat: resume it in a pty of our own, at once.
    Resume,
    /// A live process holds the chat and offers no way in. The one honest
    /// "busy": wait for the process to go, or for its inbox to appear.
    Busy,
}

/// The whole decision, as a function of the registry's answer. Total, and
/// never `Resume` for a process that is alive: that is the two-agents race
/// v34 forbade, and it stays forbidden under every other version of the
/// peer protocol, known or not.
pub fn route(live: Option<LiveSession>) -> Route {
    match live {
        None => Route::Resume,
        Some(session) if session.takes_messages() => Route::Inbox(session),
        Some(_) => Route::Busy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record as a headless 2.1.261 wrote it on 2026-09-08. Captured, and
    /// the shape below is what tech.md 6.5 names.
    const CAPTURED: &str = r#"{"pid":17607,"sessionId":"11111111-2222-4333-8444-555555555555","cwd":"/private/tmp/peertest","startedAt":1788856367901,"procStart":"Tue Sep  8 08:32:47 2026","version":"2.1.261","peerProtocol":1,"peerFeatures":["notify_idle","reply_across_default_dirs","artifact_yield"],"kind":"interactive","entrypoint":"claude-vscode","pidDomain":"darwin","name":"peertest-69","nameSource":"derived","nameSince":1788856367904,"messagingSocketPath":"/tmp/cc-socks/17607.sock","updatedAt":1788856368572}"#;

    /// The homebrew 2.1.220 of the same day: a record, but no inbox.
    const OLDER: &str = r#"{"pid":16915,"sessionId":"0c8ac7a9-b2ee-4e03-8117-a60a06b84343","cwd":"/Users/x/peekle","startedAt":1788855807577,"procStart":"Tue Sep  8 08:23:25 2026","version":"2.1.220","peerProtocol":1,"kind":"interactive","entrypoint":"cli","name":"peekle-e8","nameSource":"derived","status":"idle","updatedAt":1788855817813,"statusUpdatedAt":1788855817813}"#;

    #[test]
    fn a_captured_record_names_its_process_and_its_inbox() {
        let live = parse_record(CAPTURED).expect("parses");
        assert_eq!(live.pid, 17607);
        assert_eq!(live.session_id, "11111111-2222-4333-8444-555555555555");
        assert_eq!(live.proc_start.as_deref(), Some("Tue Sep  8 08:32:47 2026"));
        assert_eq!(live.entrypoint.as_deref(), Some("claude-vscode"));
        assert_eq!(
            live.inbox.as_deref(),
            Some(Path::new("/tmp/cc-socks/17607.sock"))
        );
        assert!(live.takes_messages());
    }

    #[test]
    fn a_record_without_a_socket_takes_no_messages() {
        let live = parse_record(OLDER).expect("parses");
        assert_eq!(live.inbox, None);
        assert!(!live.takes_messages());
    }

    #[test]
    fn only_pid_named_json_files_are_records() {
        assert!(is_record_name(Path::new("/x/17607.json")));
        assert!(!is_record_name(Path::new("/x/17607.abc.key")));
        assert!(!is_record_name(Path::new("/x/notes.json")));
        assert!(!is_record_name(Path::new("/x/.json")));
    }

    #[test]
    fn junk_is_not_a_record() {
        for junk in [
            "",
            "{}",
            "[]",
            "{\"pid\":1}",
            "{\"sessionId\":\"\",\"pid\":1}",
            "not json",
        ] {
            assert!(parse_record(junk).is_none(), "{junk}");
        }
    }

    #[test]
    fn start_times_compare_without_regard_to_padding() {
        assert!(same_start(
            "Tue Sep  8 08:32:47 2026",
            "Tue Sep 8 08:32:47 2026"
        ));
        assert!(!same_start(
            "Tue Sep  8 08:32:47 2026",
            "Tue Sep  8 08:32:48 2026"
        ));
    }

    #[test]
    fn the_process_running_this_test_is_alive_and_a_dead_pid_is_not() {
        let me = LiveSession {
            pid: std::process::id(),
            session_id: "s".into(),
            cwd: String::new(),
            proc_start: None,
            version: String::new(),
            entrypoint: None,
            peer_protocol: Some(1),
            inbox: None,
        };
        assert!(process_alive(&me));
        // A start time nobody has: the pid exists, but it is not that process.
        // Only unix can tell: there is no `ps` on Windows, and there an
        // existing pid counts as alive. tech.md 6.27 and R-22.
        #[cfg(unix)]
        {
            let reused = LiveSession {
                proc_start: Some("Mon Jan  1 00:00:00 1990".into()),
                ..me.clone()
            };
            assert!(!process_alive(&reused));
        }
        let gone = LiveSession {
            pid: u32::MAX - 1,
            ..me
        };
        assert!(!process_alive(&gone));
    }

    /// The record's clock is UTC and `ps` answers in the local one. A start
    /// written the way Claude Code writes it has to read as this process,
    /// whatever zone the machine is in. Seen live 2026-09-09. Unix only:
    /// the clock being compared is `ps`. tech.md 6.27.
    #[cfg(unix)]
    #[test]
    fn a_start_written_in_utc_names_this_process() {
        let Some(start) = utc_start_of(std::process::id()) else {
            return;
        };
        let me = LiveSession {
            pid: std::process::id(),
            session_id: "s".into(),
            cwd: String::new(),
            proc_start: Some(start),
            version: String::new(),
            entrypoint: Some("claude-vscode".into()),
            peer_protocol: Some(1),
            inbox: None,
        };
        assert!(process_alive(&me));
    }
}
