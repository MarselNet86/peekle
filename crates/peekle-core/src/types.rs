//! Core types. tech.md section 6.3 is the source of truth; this file is its
//! literal transcription. ts-rs exports every type to the frontend.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionRef {
    pub session_id: String,
    pub cwd: String,
    pub project: String,
    /// Pid of the agent. Only the hook script can supply it, because it runs as
    /// a child of the Claude process. tech.md 6.1.
    pub pid: Option<u32>,
    /// "/dev/ttys004", or None when the agent talks over pipes, as it does in
    /// an IDE extension. tech.md 6.5.
    pub tty: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PromptKind {
    Permission,
    Idle,
}

/// Where a session came from. Decides exactly one thing: whether it has an
/// input field. tech.md 6.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SessionOrigin {
    /// Peekle started it and holds its pty, so the dialogue goes both ways.
    Owned,
    /// Started outside the island. Feed and permissions, no input: there is no
    /// supported way to type into a process we did not start, and every
    /// workaround for it cost more than it gave. tech.md 6.5.
    Observed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptRequest {
    /// ulid, minted by peekle.
    pub id: String,
    pub kind: PromptKind,
    pub session: SessionRef,
    /// One line, for example "Claude finished".
    pub title: String,
    /// last_assistant_message, truncated to 2000 characters.
    pub last_message: Option<String>,
    /// Tool name and input preview, 400 characters.
    pub detail: Option<String>,
    /// Empty means free text only.
    pub options: Vec<ChoiceOption>,
    pub allow_free_text: bool,
    /// unix ms
    #[ts(type = "number")]
    pub created_at: i64,
    /// unix ms
    #[ts(type = "number")]
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub hint: Option<String>,
    pub kind: ChoiceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ChoiceKind {
    AllowOnce,
    AllowAlways,
    Deny,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptAnswer {
    pub prompt_id: String,
    pub choice: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PromptOutcome {
    Answered(PromptAnswer),
    Dismissed,
    TimedOut,
    Bypassed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    pub label: TaskLabel,
    pub status: TaskStatus,
    pub session_id: String,
    /// unix ms
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TaskLabel {
    Code,
    Fix,
    Bug,
    Test,
    Docs,
    Chore,
    Research,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TaskStatus {
    Pending,
    Active,
    Done,
}

/// What the island is showing right now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum IslandView {
    /// A notch like any other notch: the window is transparent end to end.
    #[default]
    Collapsed,
    /// One line of status, width follows the content.
    Pill,
    /// The list of sessions.
    Sessions,
    /// The feed of one session, by its session_id.
    Session(String),
}

impl IslandView {
    /// Whether this view takes clicks. Collapsed and Pill are output only, and
    /// a transparent window that swallows clicks makes the desktop unusable.
    /// tech.md 6.7.
    pub fn takes_clicks(&self) -> bool {
        !matches!(self, Self::Collapsed | Self::Pill)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EntryKind {
    User,
    Assistant,
    Tool,
    /// The agent reasoning with itself, collapsed to a marker. Claude Code
    /// shows the same thing in the terminal. tech.md 6.11.
    Thought,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EntryState {
    Running,
    Ok,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FeedEntry {
    /// ulid
    pub id: String,
    pub kind: EntryKind,
    /// A turn, a reply, or the input preview of a tool call.
    pub text: String,
    /// Tool name, set for `EntryKind::Tool`.
    pub tool: Option<String>,
    /// The body behind an expansion: the input and output of a tool call, or
    /// the reasoning behind a `Thought`. tech.md 6.3.
    pub detail: Option<String>,
    pub state: EntryState,
    /// unix ms
    #[ts(type = "number")]
    pub at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SessionStatus {
    Working,
    Idle,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionCard {
    pub session: SessionRef,
    /// First user turn, truncated to 80.
    pub title: String,
    pub status: SessionStatus,
    pub origin: SessionOrigin,
    /// Tail of the feed, capped at 200 per session.
    pub entries: Vec<FeedEntry>,
    /// unix ms
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum UsageWindow {
    FiveHour,
    SevenDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UsageWindowStat {
    pub window: UsageWindow,
    /// 0.0 .. 100.0
    pub used_pct: f32,
    /// unix seconds
    #[ts(type = "number | null")]
    pub resets_at: Option<i64>,
}

impl UsageWindowStat {
    /// The only constructor. Rate limit headers are undocumented and can hand
    /// back anything, so the clamp lives here rather than in every caller.
    pub fn new(window: UsageWindow, used_pct: f32, resets_at: Option<i64>) -> Self {
        Self {
            window,
            used_pct: clamp_pct(used_pct),
            resets_at,
        }
    }
}

/// Clamps to 0..=100. NaN reads as 0, because a bar with no number is honest
/// and a bar with a NaN width is not.
pub fn clamp_pct(pct: f32) -> f32 {
    if pct.is_nan() {
        0.0
    } else {
        pct.clamp(0.0, 100.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum UsageSource {
    Account,
    Fake,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum UsageUnavailable {
    Disabled,
    NotGranted,
    Denied,
    NotLoggedIn,
    Network,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UsageSnapshot {
    pub windows: Vec<UsageWindowStat>,
    pub source: UsageSource,
    pub reason: Option<UsageUnavailable>,
    /// unix ms
    #[ts(type = "number")]
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ToastTone {
    Neutral,
    On,
    Off,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ToastRequest {
    pub text: String,
    pub tone: ToastTone,
    pub ttl_ms: u32,
    pub badge: Option<u32>,
}

/// A screenshot sitting on the pasteboard, offered to a session.
///
/// The image itself is not in here and never will be: `PeekleState` is
/// serialized whole on every event, and the offer is answered by a key rather
/// than by looking at a preview. tech.md 6.13.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShotOffer {
    /// ulid, minted by peekle. Names the file it becomes.
    pub id: String,
    /// The owned session it would attach to.
    pub session_id: String,
    /// Project of that session, so the offer names where the shot is going.
    pub project: String,
    /// unix ms
    #[ts(type = "number")]
    pub created_at: i64,
    /// unix ms
    #[ts(type = "number")]
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PeekleState {
    pub enabled: bool,
    pub view: IslandView,
    pub active_prompt: Option<PromptRequest>,
    /// Freshest activity first, capped at 20.
    pub sessions: Vec<SessionCard>,
    /// Freshest activity first, capped at 50.
    pub tasks: Vec<TaskItem>,
    pub usage: UsageSnapshot,
    /// The screenshot offer standing right now. Lives seconds, and only
    /// one stands at a time. tech.md 6.13.
    pub shot: Option<ShotOffer>,
    pub live_sessions: u32,
    /// false when the combination is held by another application.
    pub hotkey_ok: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_out_of_range_utilization() {
        assert_eq!(clamp_pct(-4.0), 0.0);
        assert_eq!(clamp_pct(140.0), 100.0);
        assert_eq!(clamp_pct(f32::NAN), 0.0);
        assert_eq!(clamp_pct(37.5), 37.5);
    }

    #[test]
    fn window_stat_constructor_clamps() {
        let stat = UsageWindowStat::new(UsageWindow::FiveHour, 250.0, None);
        assert_eq!(stat.used_pct, 100.0);
    }
}
