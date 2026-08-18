//! Session registry. tech.md 6.3 is the source of truth for the types, the
//! caps and the rule that drives `EntryState`.
//!
//! The registry is pure: it takes already parsed events and holds the feed. It
//! never reads a transcript file, because that file is written asynchronously
//! and lags the live turn. tech.md section 8.

use std::collections::HashMap;

use serde_json::Value;
use ulid::Ulid;

use crate::types::{EntryKind, EntryState, FeedEntry, SessionCard, SessionRef, SessionStatus};

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
    pub fn from_payload(payload: &Value) -> Option<Self> {
        let event = str_at(payload, "hook_event_name")?;

        match event {
            "UserPromptSubmit" => Some(Self::UserTurn {
                session: session_ref(payload),
                text: str_at(payload, "prompt")?.to_string(),
            }),
            "PreToolUse" => Some(Self::ToolStarted {
                session: session_ref(payload),
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

fn session_ref(payload: &Value) -> SessionRef {
    let cwd = str_at(payload, "cwd").unwrap_or_default().to_string();
    SessionRef {
        session_id: str_at(payload, "session_id")
            .unwrap_or_default()
            .to_string(),
        project: project_of(&cwd),
        cwd,
    }
}

/// The last path segment. A full path does not fit a session row and the
/// directory name is what the user calls the project.
fn project_of(cwd: &str) -> String {
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
        state,
        at,
    }
}

/// Owns every session card and the feed inside it.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    cards: Vec<SessionCard>,
    /// `tool_use_id` to the session and entry it opened. Deliberately outside
    /// `PeekleState`: `FeedEntry::id` stays a ulid and the frontend never sees
    /// this map. tech.md 6.3.
    open_tools: HashMap<String, (String, String)>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cards(&self) -> &[SessionCard] {
        &self.cards
    }

    pub fn apply(&mut self, event: FeedEvent, at: i64) {
        match event {
            FeedEvent::UserTurn { session, text } => {
                let entry = now_entry(
                    EntryKind::User,
                    truncate(&text, PREVIEW_LIMIT),
                    None,
                    EntryState::Ok,
                    at,
                );
                let card = self.card_mut(session, at);
                if card.title.is_empty() {
                    card.title = truncate(&text, TITLE_LIMIT);
                }
                card.status = SessionStatus::Working;
                push_entry(card, entry);
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

    /// Moves a session to a status. Returns false when the session is unknown,
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
                    entries: Vec::new(),
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
