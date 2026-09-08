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
    /// `AskUserQuestion`, arriving through `/permission` in this environment
    /// rather than through `PreToolUse` the way the plain CLI documents it.
    /// tech.md 6.13's sibling, one to four multiple-choice questions rather
    /// than an allow/deny pair. tech.md 6.14.
    Question,
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
    /// Empty means free text only. Unused for `PromptKind::Question`, which
    /// answers through `questions` instead: its options are not a flat list,
    /// each question carries its own. tech.md 6.14.
    pub options: Vec<ChoiceOption>,
    /// Populated only for `PromptKind::Question`. Empty everywhere else.
    /// tech.md 6.14.
    pub questions: Vec<Question>,
    pub allow_free_text: bool,
    /// unix ms
    #[ts(type = "number")]
    pub created_at: i64,
    /// unix ms
    #[ts(type = "number")]
    pub expires_at: i64,
}

/// One of the one to four multiple-choice questions an `AskUserQuestion`
/// carries. tech.md 6.14.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Question {
    /// Short label, for example "Framework".
    pub header: String,
    /// The question itself, verbatim: it is also the key the answer is
    /// reported under, so it travels unshortened. tech.md 6.14.
    pub question: String,
    pub options: Vec<QuestionOption>,
    /// More than one answer may be chosen. tech.md 6.14.
    pub multi_select: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QuestionOption {
    pub label: String,
    pub description: Option<String>,
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

/// One question of a `PromptKind::Question` request, answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QuestionAnswer {
    /// The question text this answers, matched by value against
    /// `Question::question` on the way out. tech.md 6.14.
    pub question: String,
    /// More than one only when the question was `multi_select`.
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptAnswer {
    pub prompt_id: String,
    pub choice: Option<String>,
    pub text: Option<String>,
    /// Populated only answering a `PromptKind::Question`. Empty everywhere
    /// else. tech.md 6.14.
    pub answers: Vec<QuestionAnswer>,
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
    /// Something the conversation records that nobody said: the model was
    /// changed here. Claude Code writes it into the transcript itself, and the
    /// feed reads it from there rather than inventing a row of its own.
    /// tech.md 6.11 and 6.15.
    Notice,
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

// No `Eq`: the card now carries a percentage, and a float has no total
// equality. Nothing compares cards for identity anyway. tech.md 6.15.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionCard {
    pub session: SessionRef,
    /// First user turn, truncated to 80.
    pub title: String,
    pub status: SessionStatus,
    pub origin: SessionOrigin,
    /// Tail of the feed, capped at 200 per session.
    pub entries: Vec<FeedEntry>,
    /// What this session is answering with. `None` until its transcript names
    /// a model, which is every session that has not had a turn yet.
    /// tech.md 6.15.
    pub agent: Option<AgentSetup>,
    /// unix ms
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// The model of a session, the effort it answers with, and how full its
/// context window is. Read out of the transcript: no hook carries any of the
/// three. tech.md 6.15.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentSetup {
    /// "claude-opus-5", exactly as the transcript writes it.
    pub model: Option<String>,
    /// "Opus 5", from the captured catalog. `None` for a model the catalog
    /// does not know, and then the id is all the row can show.
    pub label: Option<String>,
    /// `None` when the model takes no effort at all, and when the file never
    /// said. tech.md 6.15.
    pub effort: Option<Effort>,
    /// What this model accepts, weakest first. Empty means it takes no effort
    /// at all, and then the menu is not offered rather than offered dead.
    /// tech.md 6.15.
    pub levels: Vec<Effort>,
    /// What the agent's last request took of the window.
    pub context_tokens: u32,
    /// What it is measured against. Never zero: a ring divided by zero is a
    /// ring that shows nothing. tech.md 6.15.
    pub context_window: u32,
    /// 0.0 .. 100.0
    pub context_pct: f32,
}

impl AgentSetup {
    /// The only constructor, so the percentage is never computed twice in two
    /// different ways.
    pub fn new(
        model: Option<String>,
        label: Option<String>,
        effort: Option<Effort>,
        levels: Vec<Effort>,
        context_tokens: u32,
        context_window: u32,
    ) -> Self {
        // A window of zero would come from a catalog we failed to read, and
        // dividing by it is worse than measuring against the default.
        let context_window = context_window.max(1);
        Self {
            model,
            label,
            effort,
            levels,
            context_tokens,
            context_window,
            context_pct: clamp_pct(context_tokens as f32 / context_window as f32 * 100.0),
        }
    }

    /// The same reading against a window the user pinned by hand.
    /// `[behavior] context_window`, tech.md 6.8 and 6.15.
    pub fn with_window(self, context_window: u32) -> Self {
        Self::new(
            self.model,
            self.label,
            self.effort,
            self.levels,
            self.context_tokens,
            context_window,
        )
    }
}

/// How hard the model is asked to think. The list is `claude --effort`, and
/// nothing outside it is sent. tech.md 6.15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Effort {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

/// One row of the model menu, from the catalog of 6.15. Handed to the island
/// by `get_models` rather than typed into the markup, so a model that arrives
/// in a captured catalog arrives in the menu with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModelChoice {
    /// What travels in the command: "opus".
    pub alias: String,
    /// What the row says: "Opus 5".
    pub label: String,
    /// The catalog id behind it, so the menu can mark the current one.
    pub id: String,
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
    /// No network at all: the name did not resolve, or nothing could be
    /// connected to. Measured, this fails in a tenth of a second, and it is
    /// the one failure a person can act on themselves -- and the only one
    /// cheap enough to retry within the same press. tech.md 6.4.
    Offline,
    /// Something is out there but did not answer, or answered with a status
    /// nothing else names. A retry costs the whole timeout and rarely helps,
    /// so a press tries this once. tech.md 6.4.
    Network,
    /// The endpoint answered `429`. Reached, understood, and asked to wait --
    /// which is the opposite of a network failure, and retrying sooner makes
    /// it worse. tech.md 6.4.
    RateLimited,
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
    /// Whether Keychain access was ever granted, ever -- not whether this
    /// particular read succeeded. A provider does not know this; it is config
    /// state stamped on at the one place snapshots are handed to the island,
    /// not baked in when the reading itself happened. tech.md 6.4.
    ///
    /// This is what the island gates the session list on rather than
    /// `source == Account`: a later rate limit or a network blip must not
    /// hide history that was already reachable once. Losing that would be
    /// the same fragility the gate exists to end.
    pub keychain_granted: bool,
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

/// How far the island's own sign-in has got. tech.md 6.16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SignInStage {
    /// Nothing is running.
    Idle,
    /// `claude auth login` is up and has printed nothing yet.
    Starting,
    /// The authorize URL is out, Claude Code has opened the browser, and the
    /// CLI is waiting on the code the page shows.
    Waiting,
    /// The code went into stdin and the CLI is trading it for tokens.
    Finishing,
    /// The process exited zero.
    Done,
    /// It exited non-zero, would not start, or was cancelled.
    Failed,
}

/// What the island knows about a sign-in in progress. tech.md 6.16.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SignInState {
    pub stage: SignInStage,
    /// The authorize address, kept because "the browser did not open" is an
    /// ordinary outcome -- another default browser, a refused `open` -- and
    /// without the address on screen there is nowhere left to go.
    pub url: Option<String>,
    /// Whether the CLI has asked for the code from the page.
    pub needs_code: bool,
    /// Why it failed, in words. Never the code and never the URL: one is a
    /// credential and the other carries `code_challenge` and `state`.
    /// tech.md 6.16 and rule 11.
    pub error: Option<String>,
}

impl SignInState {
    pub fn idle() -> Self {
        Self {
            stage: SignInStage::Idle,
            url: None,
            needs_code: false,
            error: None,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            stage: SignInStage::Failed,
            url: None,
            needs_code: false,
            error: Some(error.into()),
        }
    }
}

impl Default for SignInState {
    fn default() -> Self {
        Self::idle()
    }
}
