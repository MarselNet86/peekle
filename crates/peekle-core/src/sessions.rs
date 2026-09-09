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
    EntryKind, EntryState, FeedEntry, PermissionMode, SessionCard, SessionOrigin, SessionRef,
    SessionStatus,
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
                // false confirmation of a reply still in flight. A reply the
                // island put into a live process's inbox arrives wrapped, and
                // the words inside are the turn. tech.md 6.11.
                let text = crate::transcripts::spoken(text)?;
                Some(Self::UserTurn {
                    session: session_ref_of(payload),
                    text,
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

/// Which permission mode the payload says the session is in.
///
/// Every hook of a live session carries `permission_mode`; the ones captured
/// in `fixtures/hooks/` show `default` and `auto` on `UserPromptSubmit`,
/// `PreToolUse`, `PostToolUse`, `Stop` and `PermissionRequest`. A name this
/// version does not know reads as nothing at all. tech.md 6.19.
pub fn mode_of(payload: &Value) -> Option<PermissionMode> {
    PermissionMode::from_hook(str_at(payload, "permission_mode")?)
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
    /// Ids of User entries the hook path has already confirmed, but the
    /// transcript has not shown yet, keyed by session. tech.md 6.11 says the
    /// file lags: `UserPromptSubmit` reaches `confirm_reply` well before
    /// Claude Code's own async write does, and the moment it flips a reply
    /// from `Running` to `Ok` is the moment `adopt_entries` would otherwise
    /// stop protecting it -- state, not file presence, decided whether a row
    /// was safe to drop, and those are different questions. A row's id stays
    /// here until the file actually names it, whatever its displayed state.
    local_pending: HashMap<String, HashSet<String>>,
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
                mode: None,
                thinking: None,
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
        self.local_pending.remove(session_id);
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
                let confirmed = self.confirm_reply(&session_id, &text, at);

                let entry = (!confirmed).then(|| {
                    now_entry(
                        EntryKind::User,
                        truncate(&text, PREVIEW_LIMIT),
                        None,
                        EntryState::Ok,
                        at,
                    )
                });
                // Same protection as a reply the island typed itself: the
                // hook is what said this happened, the file has not shown it
                // yet, and it must not vanish in between. Recorded before the
                // card is borrowed, because `card_mut` takes all of `self`.
                // tech.md 6.11.
                if let Some(entry) = &entry {
                    self.local_pending
                        .entry(session_id.clone())
                        .or_default()
                        .insert(entry.id.clone());
                }
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
        let id = entry.id.clone();
        let session_id = card.session.session_id.clone();
        push_entry(card, entry);
        // The same protection a typed reply gets, and for the same reason
        // turned around: `Stop` is a blocking hook, so Claude Code writes its
        // answer to the transcript only after this hook has returned. A read
        // triggered by `Stop` therefore sees the turn without its answer, and
        // replacing the feed with that erases the answer a moment after it
        // arrived. Held until the file names it. tech.md 6.11.
        self.local_pending.entry(session_id).or_default().insert(id);
    }

    /// What the user just sent, as a feed entry.
    ///
    /// Putting it in the feed is what makes a chat a chat: a message that
    /// vanishes on submit reads as one that never went. `Running` means queued
    /// and not delivered yet, and only the `Stop` that carries it away turns it
    /// into `Ok`. tech.md 6.5.
    ///
    /// Returns the row's id, so the caller can come back for it: a reply that
    /// no `UserPromptSubmit` ever names is failed by id, never by position.
    pub fn user_turn(
        &mut self,
        session: SessionRef,
        text: &str,
        state: EntryState,
        at: i64,
    ) -> Option<String> {
        let trimmed = truncate(text, ASSISTANT_LIMIT);
        if trimmed.is_empty() {
            return None;
        }
        let entry = now_entry(EntryKind::User, trimmed, None, state, at);
        let entry_id = entry.id.clone();
        // A message still on its way to the agent is one the transcript is
        // going to name and has not named yet, so `adopt_entries` has to keep
        // it. Recorded by id and read by id, never by `state`: confirming
        // delivery flips `Running` to `Ok`, and that must not be what ends the
        // protection, because the file catching up is a separate and later
        // event. An answer to a permission request arrives `Ok` and is not
        // recorded at all -- it is not a prompt, so no transcript row will
        // ever match it and protecting it would pin it to the feed forever.
        // Recorded before the card is borrowed, because `card_mut` takes all
        // of `self`. tech.md 6.11.
        if state == EntryState::Running {
            self.local_pending
                .entry(session.session_id.clone())
                .or_default()
                .insert(entry.id.clone());
        }
        let card = self.card_mut(session, at);
        push_entry(card, entry);
        Some(entry_id)
    }

    /// Whether one reply is still `Running`, which is what a reply nothing has
    /// confirmed looks like from outside. tech.md 6.3.
    pub fn reply_waiting(&self, session_id: &str, entry_id: &str) -> bool {
        self.cards
            .iter()
            .find(|c| c.session.session_id == session_id)
            .is_some_and(|card| {
                card.entries.iter().any(|e| {
                    e.id == entry_id && e.kind == EntryKind::User && e.state == EntryState::Running
                })
            })
    }

    /// Gives up on one reply that nothing confirmed in time. tech.md 6.3.
    ///
    /// By id and only from `Running`: a reply the hook confirmed meanwhile is
    /// left alone, and one already failed is not failed twice. The window is
    /// `delivery_confirm_secs`, and this is the only place it ends -- exactly
    /// once, whichever way the race went (rule 10).
    pub fn fail_reply(&mut self, session_id: &str, entry_id: &str, at: i64) -> bool {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        else {
            return false;
        };
        let Some(entry) = card.entries.iter_mut().find(|e| {
            e.id == entry_id && e.kind == EntryKind::User && e.state == EntryState::Running
        }) else {
            return false;
        };
        entry.state = EntryState::Failed;
        card.updated_at = at;
        true
    }

    /// Replaces a session's feed with what its transcript says. tech.md 6.11.
    ///
    /// The file is the record Claude Code keeps of the same conversation, and
    /// it carries what no hook does: the words between the tool calls. Hooks
    /// are faster and the file is right, so the file wins on everything it
    /// knows about.
    ///
    /// One thing survives it: a message the island sent, or a hook reported,
    /// that the file has not caught up with yet. Those are tracked by id, so
    /// a reply stays protected across the `UserPromptSubmit` that confirms
    /// delivery -- being confirmed says the agent has it, not that the file
    /// does. Dropping it here would take a sent message off the screen and
    /// put it back a turn later under a different id. It goes back at the
    /// time it was sent rather than at the end (`weave`): the file keeps
    /// being written after a message it has not named, so appending one would
    /// slide it under an answer to a later turn. Nothing else is kept,
    /// because everything else came from the file to begin with.
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
        let still_local = self
            .local_pending
            .entry(session_id.to_string())
            .or_default();
        let pending: Vec<FeedEntry> = card
            .entries
            .iter()
            // Locally authored and not yet named by the file -- tracked by
            // id, not by `state`. A reply the hook already confirmed
            // (Running -> Ok) is exactly as unwritten to disk as one that
            // has not been confirmed yet; the file lags either way.
            // tech.md 6.11.
            .filter(|entry| {
                matches!(entry.kind, EntryKind::User | EntryKind::Assistant)
                    && still_local.contains(&entry.id)
            })
            // Already in the file under its own id, so the local copy has done
            // its job and would only stand there twice.
            .filter(|entry| {
                !adopted
                    .iter()
                    .any(|each| same_saying(&each.text, &entry.text))
            })
            .cloned()
            .collect();

        // Resolved either way: matched by the file just now (dropped out of
        // `pending` above), or not present in `pending` at all because it
        // fell off the cap before the file ever caught up. Either way there
        // is nothing left here worth protecting on the next read.
        let carried: HashSet<&str> = pending.iter().map(|entry| entry.id.as_str()).collect();
        still_local.retain(|id| carried.contains(id.as_str()));
        if still_local.is_empty() {
            self.local_pending.remove(session_id);
        }

        adopted = weave(adopted, pending);

        if adopted.len() > ENTRY_CAP {
            adopted.drain(..adopted.len() - ENTRY_CAP);
        }
        // A turn that ended in an API error ends without a `Stop`: nothing
        // fires, and the card would spin `Working` until the stale sweep
        // gave up on it ten minutes later. The file knows the turn is over,
        // and the file is right. tech.md 6.11.
        let ended_in_error = adopted.last().is_some_and(|last| {
            last.kind == EntryKind::Assistant && last.state == EntryState::Failed
        });
        if ended_in_error && card.status == SessionStatus::Working {
            card.status = SessionStatus::Idle;
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
    /// one. The hook carries the prompt, so the row it names is the one with
    /// those words -- and a row already given up on is set right by them,
    /// because arriving late is still arriving. Only when no row carries the
    /// words does the channel's FIFO order decide: the oldest reply still
    /// waiting is the one this event belongs to. Returns whether it found one,
    /// because the caller then knows not to add a row of its own. tech.md 6.3.
    pub fn confirm_reply(&mut self, session_id: &str, text: &str, at: i64) -> bool {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|c| c.session.session_id == session_id)
        else {
            return false;
        };
        let spoken = truncate(text, ASSISTANT_LIMIT);
        let by_words = card.entries.iter().position(|e| {
            e.kind == EntryKind::User
                && matches!(e.state, EntryState::Running | EntryState::Failed)
                && e.text == spoken
        });
        let by_order = || {
            card.entries
                .iter()
                .position(|e| e.kind == EntryKind::User && e.state == EntryState::Running)
        };
        let Some(index) = by_words.or_else(by_order) else {
            return false;
        };
        card.entries[index].state = EntryState::Ok;
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
    /// Records the permission mode a hook reported. `true` when it changed,
    /// so a card only travels to the island when there is news. tech.md 6.19.
    pub fn set_mode(&mut self, session_id: &str, mode: PermissionMode) -> bool {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|card| card.session.session_id == session_id)
        else {
            return false;
        };
        if card.mode == Some(mode) {
            return false;
        }
        card.mode = Some(mode);
        true
    }

    /// Records what a session Peekle started was given for thinking. Nothing
    /// else sets it: for a session started elsewhere the answer is unknown,
    /// and unknown is not `false`. tech.md 6.20.
    pub fn set_thinking(&mut self, session_id: &str, thinking: bool) -> bool {
        let Some(card) = self
            .cards
            .iter_mut()
            .find(|card| card.session.session_id == session_id)
        else {
            return false;
        };
        if card.thinking == Some(thinking) {
            return false;
        }
        card.thinking = Some(thinking);
        true
    }

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
                    mode: None,
                    thinking: None,
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
                self.local_pending.remove(&id);
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
/// Puts the rows the file has not caught up with back where they were said.
///
/// Appending them was what made a sent message jump: a reply the transcript
/// has not named yet is not the newest thing in the feed, it is only the
/// thing the file is missing, and the file goes on being written after it.
/// Two messages sent in a row and one of them written first was enough --
/// the other one landed under the agent's answer to the later one, and the
/// conversation read in the wrong order.
///
/// Both sides carry epoch milliseconds -- the transcript's `timestamp` and
/// the hook's clock -- so time is the one thing they can be lined up by. The
/// file's own order is never disturbed: a carried row goes before the first
/// row younger than it, and a tie leaves the file first, because a row the
/// file already knows about is the older event of the two. tech.md 6.11.
/// Whether two rows are the same thing said once.
///
/// Not equality: the same answer reaches the feed twice by two roads, and the
/// two roads cut it differently. `Stop` carries it capped at
/// `LAST_MESSAGE_LIMIT` with a mark on the cut, the transcript carries it
/// capped at its own limit without one. Comparing the openings answers the
/// only question being asked -- is this row already in the file -- without
/// either cap deciding it. tech.md 6.11.
fn same_saying(a: &str, b: &str) -> bool {
    /// Long enough that two different sayings cannot share it, short enough
    /// to sit well inside every cap either road applies.
    const HEAD: usize = 120;

    let head = |text: &str| -> String { text.trim().chars().take(HEAD).collect() };
    head(a) == head(b)
}

fn weave(adopted: Vec<FeedEntry>, mut pending: Vec<FeedEntry>) -> Vec<FeedEntry> {
    if pending.is_empty() {
        return adopted;
    }
    pending.sort_by_key(|entry| entry.at);

    let mut woven = Vec::with_capacity(adopted.len() + pending.len());
    let mut carried = pending.into_iter().peekable();
    for entry in adopted {
        while let Some(local) = carried.next_if(|local| local.at < entry.at) {
            woven.push(local);
        }
        woven.push(entry);
    }
    woven.extend(carried);
    woven
}

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
