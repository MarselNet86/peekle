//! Wires the hook server to the app. Implements `HookSink` from peekle-server.
//!
//! Every field lookup here is optional. Claude Code adds fields between
//! versions, and the exact task payloads are pinned by captured fixtures
//! rather than by documentation. tech.md R-4.

use std::sync::Arc;
use std::time::Duration;

use peekle_core::labels::classify;
use peekle_core::types::{
    IslandView, PromptOutcome, PromptRequest, SessionStatus, TaskItem, TaskStatus, ToastRequest,
    ToastTone,
};
use peekle_core::FeedEvent;
use peekle_server::HookSink;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::events;
use crate::notify;
use crate::state::AppState;
use crate::windows;

/// How long the end of an observed turn stands in the notch.
///
/// A line of prose to read and a decision to make about it, so longer than the
/// 2.6s a status toast gets, and far short of the panel's ten. tech.md 6.2.
const TURN_NOTICE_MS: u32 = 4500;

pub struct AppSink {
    app: AppHandle,
    state: Arc<AppState>,
}

impl AppSink {
    pub fn new(app: AppHandle, state: Arc<AppState>) -> Self {
        Self { app, state }
    }

    /// Re-reads the session's transcript and replaces its feed with it.
    ///
    /// Hooks carry events; the words an agent writes between its tool calls
    /// are in no hook at all and only in this file. Every payload names it, so
    /// there is nothing to guess. tech.md 6.11.
    ///
    /// Off the response path, always: the agent is standing still until the
    /// hook answers, and parsing a megabyte of JSON is not something to make
    /// it wait for. A file that will not read leaves the feed exactly as the
    /// hooks assembled it.
    fn refresh_from_transcript(&self, payload: &Value) {
        let (Some(session_id), Some(path)) = (
            payload.get("session_id").and_then(Value::as_str),
            payload.get("transcript_path").and_then(Value::as_str),
        ) else {
            return;
        };

        refresh_session(
            self.app.clone(),
            Arc::clone(&self.state),
            session_id.to_string(),
            path.to_string(),
        );
    }

    /// The same read, once the transcript has had time to be written.
    ///
    /// Nothing waits on it and nothing depends on it landing: the feed is
    /// already right without it, because the answer the hook carried is held
    /// in place until the file names it. This is what fills in the model and
    /// the context, which only the file knows. tech.md 6.11.
    fn refresh_when_settled(&self, payload: &Value) {
        /// Measured: the answer appears in the file within a few tens of
        /// milliseconds of the hook returning. This is that with room to
        /// spare, and short enough that a person does not watch the row fill.
        const SETTLE_MS: u64 = 900;

        let (Some(session_id), Some(path)) = (
            payload.get("session_id").and_then(Value::as_str),
            payload.get("transcript_path").and_then(Value::as_str),
        ) else {
            return;
        };
        let (app, state) = (self.app.clone(), Arc::clone(&self.state));
        let (session_id, path) = (session_id.to_string(), path.to_string());

        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(SETTLE_MS)).await;
            refresh_session(app, state, session_id, path);
        });
    }

    /// One line in the notch: an observed turn ended, and this is what it
    /// said. tech.md 6.2.
    ///
    /// Only on a resting island. A notice that covers what the user is reading
    /// -- an open request, an open dialogue -- is worse than no notice, and
    /// `toast` takes the view outright rather than queueing behind it.
    fn say_turn_ended(&self, session: &peekle_core::types::SessionRef, payload: &Value) {
        let Some(said) = payload
            .get("last_assistant_message")
            .and_then(Value::as_str)
            .map(first_line)
            .filter(|line| !line.is_empty())
        else {
            return;
        };
        // The banner first, and on its own rules. It does not take the
        // island's view from anything, so what is on screen decides the pill
        // and not it: the only session it stays quiet for is the one already
        // open, where the person is looking. tech.md 6.17.
        let watching = matches!(
            self.state.view(),
            IslandView::Session(ref open) if open == &session.session_id
        );
        notify::say(
            &self.app,
            self.state.lock_config().notify.enabled,
            watching,
            &session.project,
            &said,
        );

        if self.state.view() != IslandView::Collapsed || self.state.active_prompt().is_some() {
            tracing::debug!("something is on screen already, so the turn stays quiet");
            return;
        }

        windows::toast(
            &self.app,
            ToastRequest {
                // Who, then what, on two lines rather than one run-on: more
                // than one project runs at a time, and the badge on a toast
                // is a count rather than a name. tech.md 6.2.
                text: session.project.clone(),
                detail: Some(said),
                took_ms: turn_took(&self.state.sessions(), &session.session_id, now_ms()),
                tone: ToastTone::Neutral,
                ttl_ms: TURN_NOTICE_MS,
                badge: None,
            },
        );
    }

    /// How long the turn took: from the last thing the person said in this
    /// chat to now.
    ///
    /// The only number a finished turn has, and the one question a notice for
    /// it answers -- how long was I away. Counted from their words rather
    /// than from the agent's first call, the same way the work line counts
    /// (6.12): they started waiting when they sent. Nothing said in this chat
    /// yet means nothing to count from, and then the pill carries no number
    /// rather than a made up one. tech.md 6.2.
    fn emit_sessions(&self, cards: Vec<peekle_core::types::SessionCard>) {
        tracing::debug!(
            sessions = cards.len(),
            entries = cards.iter().map(|c| c.entries.len()).sum::<usize>(),
            "feed updated"
        );
        if let Err(err) = self.app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }
    }
}

impl AppSink {
    /// What mode the session was in when this hook fired. Every hook of a
    /// live session says so, and the card carries the freshest answer.
    /// tech.md 6.19.
    fn note_mode(&self, payload: &Value) {
        if let Some(cards) = self.state.note_mode(payload) {
            self.emit_sessions(cards);
        }
    }

    /// Takes the panel down when the tool it is asking about has already run.
    ///
    /// Claude Code puts its own question on screen without waiting for the
    /// hook, so the answer can be given there while the island still holds
    /// the panel up. The tool runs, `PostToolUse` arrives, and what stands on
    /// screen is a question about something already over -- for five minutes,
    /// until `permission_wait_secs` gives up on it. tech.md 6.14.
    fn settle_what_ran_elsewhere(&self, payload: &Value) {
        let Some(request) = self.state.active_prompt() else {
            return;
        };
        if !answered_elsewhere(payload, &request) {
            return;
        }

        tracing::debug!(
            session = request.session.session_id,
            tool = request.tool.as_deref().unwrap_or_default(),
            "the tool ran without us, so the panel comes down"
        );
        crate::commands::settle(
            &self.app,
            &self.state,
            &request.id,
            PromptOutcome::AnsweredElsewhere,
        );
    }
}

/// How long the turn took: from the last thing the person said in this chat
/// to now.
///
/// The only number a finished turn has, and the one question a notice about it
/// answers -- how long was I away. Counted from their words rather than from
/// the agent's first call, the same way the work line counts (6.12): they
/// started waiting when they sent. A chat with nothing of theirs in it has
/// nothing to count from, and then the pill carries no number rather than an
/// invented one. tech.md 6.2.
fn turn_took(cards: &[peekle_core::types::SessionCard], session_id: &str, at: i64) -> Option<i64> {
    let card = cards
        .iter()
        .find(|card| card.session.session_id == session_id)?;
    let started = card
        .entries
        .iter()
        .rev()
        .find(|entry| entry.kind == peekle_core::types::EntryKind::User)?
        .at;

    (at > started).then_some(at - started)
}

/// Whether a `PreCompact` says a person asked for this compact.
///
/// Only the word the CLI writes counts. An automatic compact fires mid turn
/// with nobody waiting on it, and a trigger this version has not seen is not
/// a reason to open the island over somebody's screen at the end of it.
/// tech.md 6.21.
fn asked_for(payload: &Value) -> bool {
    payload.get("trigger").and_then(Value::as_str) == Some("manual")
}

/// Whether this feed payload says the standing request has been answered
/// somewhere else.
///
/// Both marks are needed and neither is enough on its own. The session,
/// because another chat finishing a tool says nothing about this one. The
/// tool name, because a turn runs tools in parallel, and a different one
/// finishing while this question waits is the ordinary case, not a reason to
/// take the question away from someone still reading it. tech.md 6.14.
fn answered_elsewhere(payload: &Value, request: &PromptRequest) -> bool {
    let field = |key: &str| payload.get(key).and_then(Value::as_str);

    field("hook_event_name") == Some("PostToolUse")
        && field("session_id") == Some(request.session.session_id.as_str())
        && request.tool.is_some()
        && field("tool_name") == request.tool.as_deref()
}

impl HookSink for AppSink {
    fn is_enabled(&self) -> bool {
        self.state.enabled()
    }

    fn open_prompt(&self, request: PromptRequest) -> oneshot::Receiver<PromptOutcome> {
        // Register before any window work: the channel has to exist before
        // anything can resolve it.
        let receiver = self.state.pending.register(request.id.clone());

        if self.state.claim_prompt(request.clone()) {
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                windows::open_prompt(&app, &request).await;
            });
        }
        receiver
    }

    /// A Stop is an event, not a decision. tech.md 6.2.
    ///
    /// It closes the rows the turn left open and drops the working status.
    /// Nothing is held and nothing is carried: text reaches an owned session
    /// through its pty the moment it is typed.
    fn on_stop(&self, payload: &Value) {
        self.note_mode(payload);
        let session = peekle_core::sessions::session_ref_of(payload);
        let at = now_ms();

        // Nothing can still be in flight once the turn is over. Whatever is
        // still Running never reported success, because PostToolUse does not
        // fire for a failed call. tech.md 6.3.
        self.state.end_turn(&session.session_id, at);
        self.state
            .set_session_status(&session, SessionStatus::Idle, at);

        // What the agent said last belongs in the feed: the user scrolls back
        // to it. tech.md S6.
        if let Some(text) = payload
            .get("last_assistant_message")
            .and_then(Value::as_str)
            .filter(|t| !t.trim().is_empty())
        {
            let text = peekle_core::truncate(text, peekle_core::LAST_MESSAGE_LIMIT);
            self.state.assistant_turn(&session, &text, at);
        }
        self.emit_sessions(self.state.sessions());
        self.refresh_from_transcript(payload);
        // And again once the file has caught up. `Stop` blocks the agent, so
        // Claude Code writes the answer, its model and what the turn cost
        // only after this hook returns: the read above sees a turn with no
        // answer in it, and the settings row would sit empty until the next
        // event, whenever that came. tech.md 6.11 and 6.15.
        self.refresh_when_settled(payload);

        // Two origins, two notices. The island owns this one, so the notch
        // opens on the dialogue: the user wrote their reply here and waits for
        // the answer here. tech.md 6.2.
        if self.state.owns_session(&session.session_id) {
            let app = self.app.clone();
            let session_id = session.session_id.clone();
            tauri::async_runtime::spawn(async move {
                windows::reveal_turn(&app, &session_id).await;
            });
        } else {
            // An observed one is read in the editor, so a panel over half the
            // screen is a hindrance. Silence is wrong too: the user walks away
            // and learns the work finished only by going back to look, which
            // is the context switch this product exists to remove. One line in
            // the notch is enough to decide whether to go and look.
            self.say_turn_ended(&session, payload);
        }

        tracing::debug!(session = %session.session_id, "the turn ended");
    }

    fn prompt_timeout(&self) -> Duration {
        self.state.prompt_timeout()
    }

    /// The router gave up waiting and already answered Claude Code as
    /// `TimedOut`. Settles the same way an explicit answer from the island
    /// would, through the very same function, so the panel and the queue do
    /// not go on believing a decision is still open after the agent has
    /// stopped waiting for one. tech.md rule 10.
    fn settle_timeout(&self, id: &str) {
        crate::commands::settle(&self.app, &self.state, id, PromptOutcome::TimedOut);
    }

    /// One endpoint, three events. UserPromptSubmit, PreToolUse and PostToolUse
    /// all land here. tech.md 6.1.
    fn on_feed(&self, payload: &Value) {
        self.note_mode(payload);
        // Before the event is read into the feed: what it settles is a
        // question standing on screen, and it settles it whatever else the
        // event turns out to carry. tech.md 6.14.
        self.settle_what_ran_elsewhere(payload);
        if let Some(event) = FeedEvent::from_payload(payload) {
            let cards = self.state.apply_feed(event, now_ms());
            self.emit_sessions(cards);
        }
        // The line above is what the event knows. This is what was actually
        // said. tech.md 6.11.
        self.refresh_from_transcript(payload);

        // A TodoWrite still carries the task list, which is a separate view of
        // the same turn. tech.md 6.3 and 6.6.
        let items = parse_tasks(payload);
        if items.is_empty() {
            return;
        }
        let merged = self.state.merge_tasks(items);
        if let Err(err) = self.app.emit(events::TASKS, &merged) {
            tracing::warn!(error = %err, "failed to emit tasks");
        }
    }

    /// SessionStart has never appeared in a capture, only SessionEnd, so a card
    /// is never created here. The registry opens one on the first feed event
    /// instead, which is the event that actually arrives. tech.md 6.1.
    fn on_session(&self, payload: &Value) {
        let Some(event) = payload.get("hook_event_name").and_then(Value::as_str) else {
            return;
        };
        let Some(session_id) = payload.get("session_id").and_then(Value::as_str) else {
            return;
        };

        match event {
            // A compact has started, and nothing will say when it ends: the
            // file will. The card carries it from here so the sign, the line
            // in the dialogue and the poll all have the same one fact to
            // stand on. tech.md 6.21.
            "PreCompact" => {
                let session = peekle_core::sessions::session_ref_of(payload);
                let manual = asked_for(payload);
                tracing::debug!(session = %session_id, manual, "a compact started");
                let cards = self.state.start_compact(&session, now_ms(), manual);
                self.emit_sessions(cards);
            }
            "SessionStart" => self.state.session_started(),
            "SessionEnd" => {
                self.state.session_ended();
                let at = now_ms();
                // A turn cannot outlive its session, so anything still running
                // never finished. tech.md 6.3.
                self.state.end_turn(session_id, at);

                // For a session Peekle owns, the process exiting is what marks
                // the card ended, and it says so from the pty. SessionEnd fires
                // at the end of any run while the client behind it carries on,
                // so treating it as the end of an owned session is exactly the
                // guess that used to kill a live input field. tech.md 6.3.
                let cards = if self.state.owns_session(session_id) {
                    tracing::debug!(session = %session_id, "a run ended, the session we own has not");
                    self.state.sessions()
                } else {
                    self.state.mark_session_ended(session_id, at)
                };
                self.emit_sessions(cards);
            }
            _ => {}
        }
    }

    fn on_notification(&self, payload: &Value) {
        let text = payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Claude needs you")
            .to_string();

        windows::toast(
            &self.app,
            ToastRequest {
                text,
                detail: None,
                took_ms: None,
                tone: ToastTone::Neutral,
                ttl_ms: 2600,
                badge: None,
            },
        );
    }
}

/// The first line with anything on it. A closing message is prose and the
/// notch is one line wide; the rest of it is in the feed of that session.
/// Reads one session's transcript and puts what it says in place of what the
/// hooks assembled. Shared by the hook path and the poll that stands in for a
/// hook when none is coming (tech.md 6.11): a turn that ends in an API error
/// ends in silence, and the file is the only place that knows.
///
/// Off the caller's thread, always: parsing a megabyte of JSON is not
/// something an agent or a ticker should wait for. A file that will not read
/// leaves the feed exactly as it was.
pub(crate) fn refresh_session(
    app: AppHandle,
    state: Arc<AppState>,
    session_id: String,
    path: String,
) {
    tauri::async_runtime::spawn_blocking(move || {
        let Ok(text) = std::fs::read_to_string(&path) else {
            tracing::debug!(path, "no transcript to read the words out of");
            return;
        };
        let Some(card) =
            peekle_core::transcripts::card_from_lines(text.lines(), &session_id, now_ms())
        else {
            // A file with no dialogue in it yet, which is ordinary in the
            // first seconds of a session and a bug at any other time.
            tracing::debug!(session = %session_id, path, "the transcript carries no dialogue");
            return;
        };
        let entries = card.entries.len();
        let model = card
            .agent
            .as_ref()
            .and_then(|setup| setup.model.clone())
            .unwrap_or_default();
        let Some(cards) = state.adopt_entries(&session_id, card.entries, card.agent) else {
            // The one way the words are read and then dropped: no card by
            // that id. Worth a line, because from the outside it looks like
            // an agent that answered into nothing.
            tracing::warn!(
                session = %session_id,
                entries,
                "read a transcript for a session no card knows"
            );
            return;
        };
        tracing::debug!(
            session = %session_id,
            entries,
            model,
            "replaced the feed from the transcript"
        );
        if let Err(err) = app.emit(events::SESSIONS, &cards) {
            tracing::warn!(error = %err, "failed to emit sessions");
        }

        // The same text answers the other question a compacting session has:
        // whether it still is. tech.md 6.21.
        settle_compact(&app, &state, &session_id, &text);
    });
}

/// Ends the compact this session was in the middle of, if the file says it is
/// over. tech.md 6.21.
///
/// No hook fires when a compact ends, so this is the only place it can end at
/// all. Three ways out and all of them clear the card: the boundary record the
/// CLI writes, a refusal it prints instead, and -- elsewhere, on the sweep --
/// a ceiling for the compact that ends in neither.
fn settle_compact(app: &AppHandle, state: &Arc<AppState>, session_id: &str, text: &str) {
    use peekle_core::transcripts::CompactState;

    let Some(run) = state.compacting(session_id) else {
        return;
    };
    let outcome = peekle_core::transcripts::compact_state(text.lines(), run.since);
    if outcome == CompactState::Running {
        return;
    }
    let Some(cards) = state.end_compact(session_id) else {
        return;
    };
    tracing::debug!(session = %session_id, done = ?outcome, "the compact is over");
    if let Err(err) = app.emit(events::SESSIONS, &cards) {
        tracing::warn!(error = %err, "failed to emit sessions");
    }

    // A compact somebody asked for ends with the island open on the row that
    // answers them: they typed the command and waited out the minutes, and
    // the answer is one line in that feed. An automatic one opens nothing --
    // nobody asked for it, it happened in the middle of a turn, and a panel
    // over half the screen at that moment is in the way. tech.md 6.21.
    if !matches!(outcome, CompactState::Done(_)) || !run.manual {
        return;
    }
    let (app, session_id) = (app.clone(), session_id.to_string());
    tauri::async_runtime::spawn(async move {
        windows::reveal_turn(&app, &session_id).await;
    });
}

pub(crate) fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn status_of(raw: Option<&str>) -> TaskStatus {
    match raw {
        Some("in_progress") => TaskStatus::Active,
        Some("completed") => TaskStatus::Done,
        _ => TaskStatus::Pending,
    }
}

/// Reads a `TodoWrite` payload into task items. An unrecognised shape yields
/// an empty list rather than a partial one, so the feed never shows debris.
fn parse_tasks(payload: &Value) -> Vec<TaskItem> {
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let Some(todos) = payload
        .get("tool_input")
        .and_then(|input| input.get("todos"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    let now = now_ms();
    todos
        .iter()
        .enumerate()
        .filter_map(|(index, todo)| {
            let title = todo.get("content").and_then(Value::as_str)?.to_string();
            Some(TaskItem {
                id: format!("{session_id}:{index}"),
                label: classify(&title),
                title,
                status: status_of(todo.get("status").and_then(Value::as_str)),
                session_id: session_id.clone(),
                updated_at: now,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The captured `PostToolUse`, as a live session sends it. Rule 6: the
    /// payload that decides this is not written by hand.
    fn captured(name: &str) -> Value {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/hooks")
            .join(format!("{name}.jsonl"));
        let raw = std::fs::read_to_string(&path).expect("fixture");
        let line = raw
            .lines()
            .find(|line| !line.trim().is_empty())
            .expect("a line");
        serde_json::from_str(line).expect("json")
    }

    /// Every line of a capture, for a fixture that carries more than one run.
    fn captured_all(name: &str) -> Vec<Value> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/hooks")
            .join(format!("{name}.jsonl"));
        let raw = std::fs::read_to_string(&path).expect("fixture");
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect()
    }

    fn standing(session: &str, tool: Option<&str>) -> PromptRequest {
        PromptRequest {
            id: "p".into(),
            kind: peekle_core::types::PromptKind::Question,
            session: peekle_core::types::SessionRef {
                session_id: session.into(),
                cwd: "/tmp".into(),
                project: "tmp".into(),
                pid: None,
                tty: None,
            },
            title: "t".into(),
            tool: tool.map(str::to_string),
            last_message: None,
            detail: None,
            options: Vec::new(),
            questions: Vec::new(),
            allow_free_text: false,
            created_at: 0,
            expires_at: 0,
        }
    }

    fn said(kind: peekle_core::types::EntryKind, at: i64) -> peekle_core::types::FeedEntry {
        peekle_core::types::FeedEntry {
            id: format!("e{at}"),
            kind,
            text: "x".into(),
            tool: None,
            detail: None,
            state: peekle_core::types::EntryState::Ok,
            at,
        }
    }

    fn chat(entries: Vec<peekle_core::types::FeedEntry>) -> peekle_core::types::SessionCard {
        peekle_core::types::SessionCard {
            session: peekle_core::types::SessionRef {
                session_id: "s".into(),
                cwd: "/tmp/peekle".into(),
                project: "peekle".into(),
                pid: None,
                tty: None,
            },
            title: "t".into(),
            status: peekle_core::types::SessionStatus::Idle,
            origin: peekle_core::types::SessionOrigin::Owned,
            entries,
            agent: None,
            mode: None,
            thinking: None,
            compacting: None,
            updated_at: 0,
        }
    }

    /// The clock the pill shows starts when the person sent, not when the
    /// agent made its first call: that whole first thought is time they
    /// waited through. tech.md 6.2 and 6.12.
    #[test]
    fn a_turn_is_timed_from_the_words_that_started_it() {
        use peekle_core::types::EntryKind;
        let cards = vec![chat(vec![
            said(EntryKind::User, 1_000),
            said(EntryKind::Assistant, 2_000),
            said(EntryKind::User, 10_000),
            said(EntryKind::Tool, 11_000),
            said(EntryKind::Assistant, 40_000),
        ])];

        assert_eq!(turn_took(&cards, "s", 42_000), Some(32_000));
    }

    /// Nothing of theirs to count from, no chat by that name, or a clock that
    /// went backwards: no number at all rather than an invented one. R-3.
    #[test]
    fn a_turn_with_nothing_to_count_from_carries_no_number() {
        use peekle_core::types::EntryKind;
        let only_agent = vec![chat(vec![said(EntryKind::Assistant, 5_000)])];

        assert_eq!(turn_took(&only_agent, "s", 9_000), None);
        assert_eq!(turn_took(&[], "s", 9_000), None);
        assert_eq!(
            turn_took(&[chat(vec![said(EntryKind::User, 9_000)])], "nobody", 9_000),
            None
        );
        // Sent in the future, by a clock that disagrees with ours.
        assert_eq!(
            turn_took(&[chat(vec![said(EntryKind::User, 9_000)])], "s", 9_000),
            None
        );
    }

    /// The bug: Claude Code shows its own question without waiting for the
    /// hook, so the answer can be given there while the panel is still up.
    /// The tool runs, and this is the event that says so. tech.md 6.14.
    #[test]
    fn the_tool_finishing_settles_the_question_it_was_asked_for() {
        let payload = captured("post_tool_use");
        let session = payload["session_id"].as_str().expect("a session");
        let tool = payload["tool_name"].as_str().expect("a tool");

        assert!(answered_elsewhere(&payload, &standing(session, Some(tool))));
    }

    /// A turn runs tools in parallel, and chats run side by side. Neither is
    /// a reason to take a question off the screen of someone reading it.
    #[test]
    fn another_tool_or_another_chat_settles_nothing() {
        let payload = captured("post_tool_use");
        let session = payload["session_id"].as_str().expect("a session");
        let tool = payload["tool_name"].as_str().expect("a tool");

        assert!(!answered_elsewhere(
            &payload,
            &standing("somebody else", Some(tool))
        ));
        assert!(!answered_elsewhere(
            &payload,
            &standing(session, Some("Write"))
        ));
        // A request about no tool at all -- the end of a turn -- is not
        // finished by any tool finishing.
        assert!(!answered_elsewhere(&payload, &standing(session, None)));
    }

    /// Only the event that means the tool is over. The call starting is the
    /// moment the question is asked, not the moment it stops mattering.
    #[test]
    fn the_call_starting_settles_nothing() {
        let payload = captured("pre_tool_use");
        let session = payload["session_id"].as_str().expect("a session");
        let tool = payload["tool_name"].as_str().expect("a tool");

        assert!(!answered_elsewhere(
            &payload,
            &standing(session, Some(tool))
        ));
    }

    #[test]
    fn reads_todos_into_tasks() {
        let payload = json!({
            "session_id": "abc",
            "tool_input": {"todos": [
                {"content": "Fix the crash", "status": "in_progress"},
                {"content": "Ship it", "status": "pending"},
            ]}
        });

        let tasks = parse_tasks(&payload);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Fix the crash");
        assert_eq!(tasks[0].status, TaskStatus::Active);
        assert_eq!(tasks[0].label, peekle_core::types::TaskLabel::Bug);
        assert_eq!(tasks[1].status, TaskStatus::Pending);
    }

    #[test]
    fn an_unrecognised_payload_yields_nothing() {
        assert!(parse_tasks(&json!({})).is_empty());
        assert!(parse_tasks(&json!({"tool_input": {}})).is_empty());
        assert!(parse_tasks(&json!({"tool_input": {"todos": "nope"}})).is_empty());
    }

    #[test]
    fn a_todo_without_content_is_skipped_not_faked() {
        let payload = json!({"tool_input": {"todos": [{"status": "pending"}]}});
        assert!(parse_tasks(&payload).is_empty());
    }

    #[test]
    fn unknown_status_falls_back_to_pending() {
        assert_eq!(status_of(Some("something_new")), TaskStatus::Pending);
        assert_eq!(status_of(None), TaskStatus::Pending);
    }

    /// The captured `PreCompact` of a `/compact` somebody typed. What turns
    /// on it is whether the island opens itself when the compact is over,
    /// which is exactly the kind of thing that must not be guessed.
    /// tech.md 6.21.
    #[test]
    fn a_compact_a_person_asked_for_is_told_from_one_the_window_forced() {
        for payload in captured_all("pre_compact") {
            assert!(asked_for(&payload), "the capture is of a typed /compact");
        }

        assert!(!asked_for(&json!({"trigger": "auto"})));
        assert!(!asked_for(&json!({})), "no trigger is not a person asking");
    }
}
