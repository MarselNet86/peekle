//! Session registry. tech.md 6.3 is the source of truth for the types, the
//! caps and the rule that drives `EntryState`.
//!
//! The registry is pure: it takes already parsed events and holds the feed. It
//! never reads a transcript file, because that file is written asynchronously
//! and lags the live turn. tech.md section 8.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ulid::Ulid;

use crate::types::{
    EntryKind, EntryState, FeedEntry, SessionCard, SessionOrigin, SessionRef, SessionStatus,
};

/// Freshest activity first, capped. tech.md 6.3.
pub const SESSION_CAP: usize = 20;
/// Tail of the feed per session. tech.md 6.3.
pub const ENTRY_CAP: usize = 200;

/// First user turn becomes the title. tech.md 6.3.
const TITLE_LIMIT: usize = 80;
/// Tool input preview, same budget as `PromptRequest::detail`. tech.md 6.3.
const PREVIEW_LIMIT: usize = 400;
/// Same budget the Stop mapping already applies to last_assistant_message.
const ASSISTANT_LIMIT: usize = 2000;

/// Keys worth showing before falling back to the whole input. A tool call reads
/// as its subject, not as its JSON, and every one of these carries the subject.
const PREVIEW_KEYS: &[&str] = &[
    "command",
    "file_path",
    "path",
    "pattern",
    "query",
    "url",
    "prompt",
    "description",
];

/// What a payload on `/feed` turned out to be. An event this build does not
/// recognise yields `None` rather than a half filled entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedEvent {
    UserTurn {
        session: SessionRef,
        text: String,
    },
    ToolStarted {
        session: SessionRef,
        tool_use_id: String,
        tool: String,
        preview: String,
    },
    ToolFinished {
        session_id: String,
        tool_use_id: String,
    },
}

impl FeedEvent {
    /// The session this event belongs to.
    pub fn session_id(&self) -> &str {
        match self {
            FeedEvent::UserTurn { session, .. } => &session.session_id,
            FeedEvent::ToolStarted { session, .. } => &session.session_id,
            FeedEvent::ToolFinished { session_id, .. } => session_id,
        }
    }

    pub fn from_payload(payload: &Value) -> Option<Self> {
        let event = str_at(payload, "hook_event_name")?;

        match event {
            "UserPromptSubmit" => {
                let text = str_at(payload, "prompt")?;
                // Not everything arriving as the user was said by one. Dropped
                // here, before the event exists, so none of what follows
                // happens: no green bubble, no title taken from it, and no
                // false confirmation of a reply still in flight. tech.md 6.11.
                if crate::transcripts::is_synthetic(text) {
                    return None;
                }
                Some(Self::UserTurn {
                    session: session_ref_of(payload),
                    text: text.to_string(),
                })
            }
            "PreToolUse" => Some(Self::ToolStarted {
                session: session_ref_of(payload),
                tool_use_id: str_at(payload, "tool_use_id")?.to_string(),
                tool: str_at(payload, "tool_name")?.to_string(),
                preview: preview_of(payload.get("tool_input")),
            }),
            // The payload carries the whole input again, but the row already
            // has it from PreToolUse, so nothing here is re-read.
            "PostToolUse" => Some(Self::ToolFinished {
                session_id: str_at(payload, "session_id")?.to_string(),
                tool_use_id: str_at(payload, "tool_use_id")?.to_string(),
            }),
            _ => None,
        }
    }
}

fn str_at<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Value::as_str)
}

/// The session a hook payload is about. Public because the Stop path needs the
/// same reading of the same fields the feed uses. tech.md 6.3.
pub fn session_ref_of(payload: &Value) -> SessionRef {
    let cwd = str_at(payload, "cwd").unwrap_or_default().to_string();
    SessionRef {
        session_id: str_at(payload, "session_id")
            .unwrap_or_default()
            .to_string(),
        project: project_of(&cwd),
        cwd,
        // Only the hook script knows these, and only for live events. A
        // payload without them is not an error: backfilled and older sessions
        // simply take the turn-boundary path. tech.md 6.1.
        pid: payload.get("pid").and_then(Value::as_u64).map(|p| p as u32),
        tty: str_at(payload, "tty").map(str::to_string),
    }
}

/// The last path segment. A full path does not fit a session row and the
/// directory name is what the user calls the project.
pub fn project_of(cwd: &str) -> String {
    cwd.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// Truncates on a character boundary. Byte slicing panics on any prompt that is
/// not ASCII, and prompts are prose.
fn truncate(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }
    trimmed.chars().take(limit).collect()
}

/// One readable line for a tool call. Falls back to the compact JSON so an
/// unknown tool still shows something true rather than an empty row.
fn preview_of(input: Option<&Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };

    let text = PREVIEW_KEYS
        .iter()
        .find_map(|key| input.get(key).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| input.to_string());

    truncate(&text, PREVIEW_LIMIT)
}

fn now_entry(
    kind: EntryKind,
    text: String,
    tool: Option<String>,
    state: EntryState,
    at: i64,
) -> FeedEntry {
    FeedEntry {
        id: Ulid::generate().to_string(),
        kind,
        text,
        tool,
        // The live path has the input already flattened into `text`. A full
        // body arrives only from a transcript. tech.md 6.3.
        detail: None,
        state,
        at,
    }
}

/// What the user said about a session, on top of what the hooks say.
///
/// Kept beside the cards rather than written into them: hooks and the
/// transcript backfill rebuild a card on every event, and a title written into
/// one would be gone by the next `PreToolUse`. tech.md 6.3.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SessionOverrides {
    /// Titles the user typed, by session id.
    #[serde(default)]
    pub titles: HashMap<String, String>,
    /// Sessions the user put away. They never come back, including from the
    /// backfill, and their transcript is untouched. tech.md 11.
    #[serde(default)]
    pub hidden: HashSet<String>,
}

/// Owns every session card and the feed inside it.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    cards: Vec<SessionCard>,
    overrides: SessionOverrides,
    /// `tool_use_id` to the session and entry it opened. Deliberately outside
    /// `PeekleState`: `FeedEntry::id` stays a ulid and the frontend never sees
    /// this map. tech.md 6.3.
    open_tools: HashMap<String, (String, String)>,
    /// Ids Peekle assigned when it started a session itself. A hook carrying
    /// one of these describes a session we own; anything else is observed.
    /// tech.md 6.5.
    owned: HashSet<String>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Claims an id before the session behind it starts talking. Called with
    /// the id passed to `claude --session-id`, so the first hook to arrive is
    /// already recognised as ours.
    pub fn claim(&mut self, session_id: &str) {
        self.owned.insert(session_id.to_string());
        if let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        {
            card.origin = SessionOrigin::Owned;
        }
    }

    /// Opens a card for a session Peekle has just started.
    ///
    /// The card has to exist before the first hook arrives. The island opens a
    /// session the moment it starts one, and a view pointing at a card that is
    /// not there yet has nothing to draw. `Idle` because nothing is running
    /// yet: the agent is waiting to be spoken to. tech.md 6.5.
    pub fn open_owned(&mut self, session: SessionRef, at: i64) {
        let id = session.session_id.clone();
        self.owned.insert(id.clone());

        if self.cards.iter().any(|c| c.session.session_id == id) {
            self.touch(&id);
            return;
        }
        self.cards.insert(
            0,
            SessionCard {
                session,
                title: String::new(),
                status: SessionStatus::Idle,
                origin: SessionOrigin::Owned,
                entries: Vec::new(),
                // Nothing has answered yet, so there is nothing to say about
                // the model. tech.md 6.15.
                agent: None,
                updated_at: at,
            },
        );
        self.evict_sessions();
    }

    /// Drops a claim. The card stays: the conversation is still worth reading
    /// after the process behind it is gone.
    pub fn disown(&mut self, session_id: &str) {
        self.owned.remove(session_id);
    }

    pub fn is_owned(&self, session_id: &str) -> bool {
        self.owned.contains(session_id)
    }

    /// Loads what the user said about sessions.
    pub fn restore(&mut self, overrides: SessionOverrides) {
        self.overrides = overrides;
        self.cards
            .retain(|c| !self.overrides.hidden.contains(&c.session.session_id));
    }

    pub fn overrides(&self) -> &SessionOverrides {
        &self.overrides
    }

    /// Renames a session, or drops the override when the title is empty.
    /// Returns false when nobody knows the session, so the caller can say so
    /// rather than storing a title for a card that does not exist.
    pub fn rename(&mut self, session_id: &str, title: &str) -> bool {
        let title = truncate(title, TITLE_LIMIT);
        let known = self
            .cards
            .iter()
            .any(|c| c.session.session_id == session_id);
        if !known {
            return false;
        }

        if title.is_empty() {
            self.overrides.titles.remove(session_id);
        } else {
            self.overrides.titles.insert(session_id.to_string(), title);
        }
        true
    }

    /// Puts a session away for good. The transcript is not touched: those
    /// files belong to Claude Code and Peekle only reads them. tech.md 11.
    pub fn hide(&mut self, session_id: &str) {
        self.overrides.hidden.insert(session_id.to_string());
        self.cards.retain(|c| c.session.session_id != session_id);
    }

    /// The cards as the user sees them: their titles over the hooks' titles,
    /// and nothing they put away.
    ///
    /// Applied on the way out rather than written in. A title written into a
    /// card would destroy the one the hooks derived, and clearing the override
    /// could then never hand it back. tech.md 6.3.
    pub fn cards(&self) -> Vec<SessionCard> {
        self.cards
            .iter()
            .filter(|card| !self.overrides.hidden.contains(&card.session.session_id))
            .map(
                |card| match self.overrides.titles.get(&card.session.session_id) {
                    Some(title) => SessionCard {
                        title: title.clone(),
                        ..card.clone()
                    },
                    None => card.clone(),
                },
            )
            .collect()
    }

    pub fn apply(&mut self, event: FeedEvent, at: i64) {
        // A session the user put away stays away, however loudly its own hooks
        // keep arriving. tech.md 6.3.
        if self.overrides.hidden.contains(event.session_id()) {
            return;
        }
        match event {
            FeedEvent::UserTurn { session, text } => {
                // The island already drew this reply when it was typed, so the
                // hook confirms that row rather than adding a second one. Only
                // a turn the island never sent -- an observed session, or one
                // started in the terminal -- has nothing to confirm and needs
                // a row of its own. tech.md 6.3.
                let session_id = session.session_id.clone();
                let confirmed = self.confirm_reply(&session_id, at);

                let entry = (!confirmed).then(|| {
                    now_entry(
                        EntryKind::User,
                        truncate(&text, PREVIEW_LIMIT),
                        None,
                        EntryState::Ok,
                        at,
                    )
                });
                let card = self.card_mut(session, at);
                if card.title.is_empty() {
                    card.title = truncate(&text, TITLE_LIMIT);
                }
                card.status = SessionStatus::Working;
                if let Some(entry) = entry {
                    push_entry(card, entry);
                }
            }
            FeedEvent::ToolStarted {
                session,
                tool_use_id,
                tool,
                preview,
            } => {
                let entry = now_entry(
                    EntryKind::Tool,
                    preview,
                    Some(tool),
                    EntryState::Running,
                    at,
                );
                let entry_id = entry.id.clone();
                let session_id = session.session_id.clone();

                let card = self.card_mut(session, at);
                card.status = SessionStatus::Working;
                let evicted = push_entry(card, entry);

                self.forget(&evicted);
                self.open_tools.insert(tool_use_id, (session_id, entry_id));
            }
            FeedEvent::ToolFinished {
                session_id,
                tool_use_id,
            } => {
                let Some((card_id, entry_id)) = self.open_tools.remove(&tool_use_id) else {
                    // The opening call fell off the cap, or Peekle started
                    // mid turn. Nothing to collapse into.
                    return;
                };
                // Trust the map over the payload: the id in it is the one that
                // actually opened the row.
                let _ = session_id;
                self.set_state(&card_id, &entry_id, EntryState::Ok, at);
            }
        }
    }

    /// What the agent said last, as a feed entry. Taken from the Stop payload
    /// and never from the transcript file: that file is written asynchronously
    /// and lags the live turn. tech.md section 8 and S6.
    pub fn assistant_turn(&mut self, session: SessionRef, text: &str, at: i64) {
        let trimmed = truncate(text, ASSISTANT_LIMIT);
        if trimmed.is_empty() {
            return;
        }

        let card = self.card_mut(session, at);
        // A Stop can repeat while the user reads it, and the same closing line
        // twice in the feed reads as the agent saying it twice.
        if card
            .entries
            .last()
            .is_some_and(|last| last.kind == EntryKind::Assistant && last.text == trimmed)
        {
            return;
        }
        let entry = now_entry(EntryKind::Assistant, trimmed, None, EntryState::Ok, at);
        push_entry(card, entry);
    }

    /// What the user just sent, as a feed entry.
    ///
    /// Putting it in the feed is what makes a chat a chat: a message that
    /// vanishes on submit reads as one that never went. `Running` means queued
    /// and not delivered yet, and only the `Stop` that carries it away turns it
    /// into `Ok`. tech.md 6.5.
    pub fn user_turn(&mut self, session: SessionRef, text: &str, state: EntryState, at: i64) {
        let trimmed = truncate(text, ASSISTANT_LIMIT);
        if trimmed.is_empty() {
            return;
        }
        let card = self.card_mut(session, at);
        let entry = now_entry(EntryKind::User, trimmed, None, state, at);
        push_entry(card, entry);
    }

    /// Replaces a session's feed with what its transcript says. tech.md 6.11.
    ///
    /// The file is the record Claude Code keeps of the same conversation, and
    /// it carries what no hook does: the words between the tool calls. Hooks
    /// are faster and the file is right, so the file wins on everything it
    /// knows about.
    ///
    /// One thing survives it: a reply typed in the island and not yet in the
    /// file. It sits `Running` until `UserPromptSubmit` confirms it, and
    /// dropping it here would take a sent message off the screen. Nothing else
    /// is kept, because everything else came from the file to begin with.
    ///
    /// False means there is no such session yet, so there is nothing to
    /// replace: a transcript never opens a card, the same rule the backfill
    /// lives by.
    pub fn adopt_entries(
        &mut self,
        session_id: &str,
        entries: Vec<FeedEntry>,
        agent: Option<crate::types::AgentSetup>,
    ) -> bool {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|card| card.session.session_id == session_id)
        else {
            return false;
        };

        let mut adopted = entries;
        let pending: Vec<FeedEntry> = card
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::User && entry.state == EntryState::Running)
            // Already in the file under its own id, so the local copy has done
            // its job and would only stand there twice.
            .filter(|entry| !adopted.iter().any(|each| each.text == entry.text))
            .cloned()
            .collect();
        adopted.extend(pending);

        if adopted.len() > ENTRY_CAP {
            adopted.drain(..adopted.len() - ENTRY_CAP);
        }
        card.entries = adopted;
        // What the file says the session answers with. A file that names no
        // model leaves the row as it stood: the transcript is written after
        // the fact, and a gap in it is not a session that lost its model.
        // tech.md 6.15.
        if agent.is_some() {
            card.agent = agent;
        }
        true
    }

    /// Marks every queued reply of a session as undeliverable.
    pub fn replies_failed(&mut self, session_id: &str, at: i64) {
        self.mark_replies(session_id, EntryState::Failed, at);
    }

    /// Confirms the oldest reply still waiting on this session, if any.
    ///
    /// `UserPromptSubmit` is what confirms a reply, and it confirms exactly
    /// one: the channel is FIFO, so the oldest unconfirmed row is the one this
    /// event belongs to. Returns whether it found one, because the caller then
    /// knows not to add a row of its own. tech.md 6.3.
    pub fn confirm_reply(&mut self, session_id: &str, at: i64) -> bool {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        else {
            return false;
        };
        let Some(entry) = card
            .entries
            .iter_mut()
            .find(|e| e.kind == EntryKind::User && e.state == EntryState::Running)
        else {
            return false;
        };
        entry.state = EntryState::Ok;
        card.updated_at = at;
        true
    }

    fn mark_replies(&mut self, session_id: &str, state: EntryState, at: i64) {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        else {
            return;
        };
        for entry in card.entries.iter_mut() {
            if entry.kind == EntryKind::User && entry.state == EntryState::Running {
                entry.state = state;
            }
        }
        card.updated_at = at;
    }

    /// Puts a session that stopped reporting back to rest.
    ///
    /// `Working` is set by an event and cleared by an event, so a session whose
    /// agent died, whose terminal was closed, or which never had an agent at
    /// all, stays working forever: the mark spins and the reply field stays
    /// dark with nothing on the way. The window is generous on purpose. A long
    /// build reports nothing between `PreToolUse` and `PostToolUse`, and
    /// calling that dead would be worse than waiting. tech.md 6.3.
    pub fn rest_stale_work(&mut self, now: i64, after: i64) -> bool {
        let mut changed = false;
        for card in self.cards.iter_mut() {
            if card.status == SessionStatus::Working && now - card.updated_at >= after {
                card.status = SessionStatus::Idle;
                changed = true;
            }
        }
        changed
    }

    /// Moves a status. Returns false when the session is unknown,
    /// which happens when Peekle started mid session.
    pub fn set_status(&mut self, session_id: &str, status: SessionStatus, at: i64) -> bool {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        else {
            return false;
        };
        card.status = status;
        card.updated_at = at;
        self.touch(session_id);
        true
    }

    /// Opens a card for a session Peekle has not seen a feed event from yet.
    /// A Stop can be the first thing that arrives if the overlay started mid
    /// turn, and a prompt with no card behind it has nothing to draw.
    pub fn ensure(&mut self, session: SessionRef, at: i64) {
        self.card_mut(session, at);
    }

    /// The turn is over, so nothing can still be in flight. Whatever is still
    /// `Running` never reported success, and `PostToolUse` does not fire for a
    /// failed call. tech.md 6.1 and 6.3.
    pub fn end_turn(&mut self, session_id: &str, at: i64) {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        else {
            return;
        };

        let mut swept = Vec::new();
        for entry in card.entries.iter_mut() {
            if entry.state == EntryState::Running {
                entry.state = EntryState::Failed;
                swept.push(entry.id.clone());
            }
        }
        if swept.is_empty() {
            return;
        }
        card.updated_at = at;
        self.open_tools
            .retain(|_, (_, entry_id)| !swept.contains(entry_id));
    }

    fn set_state(&mut self, session_id: &str, entry_id: &str, state: EntryState, at: i64) {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        else {
            return;
        };
        let Some(entry) = card.entries.iter_mut().find(|e| e.id == entry_id) else {
            return;
        };
        entry.state = state;
        card.updated_at = at;
        self.touch(session_id);
    }

    /// The card for this session, created if new. Freshest first, capped.
    fn card_mut(&mut self, session: SessionRef, at: i64) -> &mut SessionCard {
        let id = session.session_id.clone();

        if !self.cards.iter().any(|c| c.session.session_id == id) {
            self.cards.insert(
                0,
                SessionCard {
                    session,
                    title: String::new(),
                    status: SessionStatus::Working,
                    origin: if self.owned.contains(&id) {
                        SessionOrigin::Owned
                    } else {
                        SessionOrigin::Observed
                    },
                    entries: Vec::new(),
                    agent: None,
                    updated_at: at,
                },
            );
            self.evict_sessions();
        } else {
            self.touch(&id);
        }

        let index = self
            .cards
            .iter()
            .position(|c| c.session.session_id == id)
            .unwrap_or(0);
        self.cards[index].updated_at = at;
        &mut self.cards[index]
    }

    /// Adds cards the hooks have not seen, leaving everything they have seen
    /// alone.
    ///
    /// History fills the gaps and never overwrites the present: a card that
    /// arrived over a hook carries a live status and a live feed, and a file on
    /// disk knows neither. Freshest first afterwards, and the same cap as every
    /// other path in. tech.md 6.11.
    pub fn seed(&mut self, cards: Vec<SessionCard>) {
        for card in cards {
            // Put away means put away, including by the backfill that would
            // otherwise raise it again on every launch. tech.md 6.3.
            if self.overrides.hidden.contains(&card.session.session_id) {
                continue;
            }
            let known = self
                .cards
                .iter()
                .any(|c| c.session.session_id == card.session.session_id);
            if !known {
                self.cards.push(card);
            }
        }

        self.cards
            .sort_by_key(|card| std::cmp::Reverse(card.updated_at));
        self.evict_sessions();
    }

    /// Moves a session to the front. The list is ordered by activity, not by
    /// when the session started.
    fn touch(&mut self, session_id: &str) {
        let Some(index) = self
            .cards
            .iter()
            .position(|c| c.session.session_id == session_id)
        else {
            return;
        };
        if index > 0 {
            let card = self.cards.remove(index);
            self.cards.insert(0, card);
        }
    }

    fn evict_sessions(&mut self) {
        while self.cards.len() > SESSION_CAP {
            if let Some(dropped) = self.cards.pop() {
                let id = dropped.session.session_id;
                self.open_tools.retain(|_, (card_id, _)| card_id != &id);
            }
        }
    }

    fn forget(&mut self, entry_ids: &[String]) {
        if entry_ids.is_empty() {
            return;
        }
        self.open_tools
            .retain(|_, (_, entry_id)| !entry_ids.contains(entry_id));
    }
}

/// Appends and trims to the cap, returning the ids that fell off the front.
fn push_entry(card: &mut SessionCard, entry: FeedEntry) -> Vec<String> {
    card.entries.push(entry);
    if card.entries.len() <= ENTRY_CAP {
        return Vec::new();
    }
    let overflow = card.entries.len() - ENTRY_CAP;
    card.entries
        .drain(..overflow)
        .map(|entry| entry.id)
        .collect()
}
