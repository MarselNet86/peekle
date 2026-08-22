//! The one owner of application state. The frontend renders `PeekleState` and
//! sends intents; it holds no authoritative state of its own. tech.md 6.3, 8.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use peekle_core::config::Config;
use peekle_core::sessions::SessionOverrides;
use peekle_core::types::{
    IslandView, PeekleState, PromptRequest, SessionCard, SessionRef, SessionStatus, TaskItem,
    UsageSnapshot, UsageUnavailable,
};
use peekle_core::{FeedEvent, PendingRegistry, SessionRegistry};
use peekle_usage::{fake::unknown, UsageProvider};
use tokio::sync::Notify;

/// Freshest activity first, capped. tech.md 6.3.
const TASK_CAP: usize = 50;

pub struct AppState {
    pub config: Mutex<Config>,
    pub pending: PendingRegistry,
    pub usage_provider: Arc<dyn UsageProvider>,

    enabled: AtomicBool,
    view: Mutex<IslandView>,
    /// Bounds of the shape as the webview last measured them, in CSS pixels.
    /// Collapsed that rectangle is the resting mark, the one part of a resting
    /// island that takes a click; open it is what the pointer has to leave
    /// before the island puts itself away. tech.md 6.7.
    shape_bounds: Mutex<Option<(f64, f64)>>,
    /// Whether the pointer is currently inside the resting mark. Held so the
    /// tracker touches AppKit on the crossing only, not on every tick.
    over_rest: AtomicBool,
    /// Since when the pointer has been off an open island, or None while it is
    /// on it. tech.md 6.7.
    outside_since: Mutex<Option<Instant>>,
    /// Until when an island opened by a request holds itself on screen with the
    /// pointer elsewhere. tech.md 6.7.
    hold_until: Mutex<Option<Instant>>,
    hotkey_ok: AtomicBool,
    live_sessions: AtomicU32,
    active_prompt: Mutex<Option<PromptRequest>>,
    /// Blocking requests that arrived while a prompt was already open. Not part
    /// of `PeekleState`: the frontend never sees the queue. tech.md 6.3.
    queue: Mutex<Vec<PromptRequest>>,
    sessions: Mutex<SessionRegistry>,
    /// The sessions Peekle started and can type into. tech.md 6.5.
    pty: peekle_core::pty::SharedPtyHost,
    tasks: Mutex<Vec<TaskItem>>,
    usage: Mutex<UsageSnapshot>,
    ready: Mutex<HashMap<String, Arc<Notify>>>,
}

/// Whether usage may be fetched at all right now.
///
/// The Keychain half of this is rule 12: the account provider reads the
/// Keychain, so nothing may fetch on its own until the user has granted it
/// once. Every automatic path asks this first; only `request_usage_access`
/// skips it, because there the user is the one asking.
pub fn may_fetch_usage(usage: &peekle_core::config::UsageConfig) -> bool {
    use peekle_core::config::UsageProviderKind;

    if !usage.enabled || usage.provider == UsageProviderKind::Off {
        return false;
    }
    if usage.provider != UsageProviderKind::Account {
        return true;
    }
    usage.keychain_granted && !usage.keychain_denied
}

/// State, not config: `config.toml` belongs to `peekle init` and to the user,
/// and mixing their edits with ours would be a race for one file. tech.md 6.8.
fn overrides_path() -> Option<std::path::PathBuf> {
    peekle_core::config::config_path()
        .ok()?
        .parent()
        .map(|dir| dir.join("sessions.json"))
}

/// Why the bars are empty before anything has been fetched.
///
/// The first snapshot cannot arrive until the user grants Keychain access, so
/// saying `Disabled` here would tell them usage is switched off when it is
/// waiting on them. tech.md 6.4.
fn initial_reason(usage: &peekle_core::config::UsageConfig) -> UsageUnavailable {
    use peekle_core::config::UsageProviderKind;

    if !usage.enabled || usage.provider == UsageProviderKind::Off {
        return UsageUnavailable::Disabled;
    }
    if usage.provider != UsageProviderKind::Account {
        return UsageUnavailable::Unsupported;
    }
    if usage.keychain_denied {
        return UsageUnavailable::Denied;
    }
    if !usage.keychain_granted {
        return UsageUnavailable::NotGranted;
    }
    UsageUnavailable::Unsupported
}

impl AppState {
    pub fn new(config: Config, usage_provider: Arc<dyn UsageProvider>) -> Self {
        let usage_config = config.usage.clone();
        let enabled = config.behavior.enabled;
        Self {
            config: Mutex::new(config),
            pending: PendingRegistry::new(),
            usage_provider,
            enabled: AtomicBool::new(enabled),
            view: Mutex::new(IslandView::default()),
            shape_bounds: Mutex::new(None),
            over_rest: AtomicBool::new(false),
            outside_since: Mutex::new(None),
            hold_until: Mutex::new(None),
            hotkey_ok: AtomicBool::new(true),
            live_sessions: AtomicU32::new(0),
            active_prompt: Mutex::new(None),
            queue: Mutex::new(Vec::new()),
            sessions: Mutex::new(SessionRegistry::new()),
            pty: std::sync::Arc::new(peekle_core::pty::PtyHost::new()),
            tasks: Mutex::new(Vec::new()),
            usage: Mutex::new(unknown(initial_reason(&usage_config))),
            ready: Mutex::new(HashMap::new()),
        }
    }

    /// Whether an automatic path may fetch usage. tech.md 6.4 and rule 12.
    pub fn may_fetch_usage(&self) -> bool {
        may_fetch_usage(&self.lock_config().usage)
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, value: bool) {
        self.enabled.store(value, Ordering::Relaxed);
    }

    pub fn view(&self) -> IslandView {
        self.lock(&self.view).clone()
    }

    /// Stores the intent and reports whether it moved. An unchanged view must
    /// not churn the panel: toggling cursor events on a repeat is visible as a
    /// dropped click.
    pub fn set_view(&self, next: IslandView) -> bool {
        let mut view = self.lock(&self.view);
        if *view == next {
            return false;
        }
        *view = next;
        true
    }

    pub fn shape_bounds(&self) -> Option<(f64, f64)> {
        *self.lock(&self.shape_bounds)
    }

    pub fn set_shape_bounds(&self, bounds: (f64, f64)) {
        *self.lock(&self.shape_bounds) = Some(bounds);
    }

    /// Records where the pointer is and reports whether it crossed the edge.
    /// Only a crossing is worth an AppKit call.
    pub fn set_over_rest(&self, inside: bool) -> bool {
        self.over_rest.swap(inside, Ordering::SeqCst) != inside
    }

    /// Keeps a request-opened island up until `until`, pointer or no pointer.
    pub fn hold_open(&self, until: Instant) {
        *self.lock(&self.hold_until) = Some(until);
    }

    /// Whether the hold is still running. An expired hold clears itself.
    pub fn held_open(&self, now: Instant) -> bool {
        let mut hold = self.lock(&self.hold_until);
        match *hold {
            Some(until) if now < until => true,
            _ => {
                *hold = None;
                false
            }
        }
    }

    /// The user engaged, so the hold has done its job.
    pub fn clear_hold(&self) {
        *self.lock(&self.hold_until) = None;
    }

    /// The pointer is on the island, so any walking away starts over.
    pub fn pointer_returned(&self) {
        *self.lock(&self.outside_since) = None;
    }

    /// Whether the pointer has been off the island for at least `grace`. The
    /// clock starts on the first tick that finds it outside, so an island that
    /// opens under an idle pointer still gets its full grace period.
    pub fn pointer_left_for(&self, grace: Duration, now: Instant) -> bool {
        let mut since = self.lock(&self.outside_since);
        let start = since.get_or_insert(now);
        now.duration_since(*start) >= grace
    }

    /// Adds past dialogues the hooks never saw. A live session is never
    /// overwritten by a file. tech.md 6.11.
    pub fn seed_sessions(&self, cards: Vec<SessionCard>) -> Vec<SessionCard> {
        let mut sessions = self.lock(&self.sessions);
        sessions.seed(cards);
        sessions.cards().to_vec()
    }

    /// Renames a session and remembers it. False means nobody knows the id.
    pub fn rename_session(&self, session_id: &str, title: &str) -> Option<Vec<SessionCard>> {
        let mut sessions = self.lock(&self.sessions);
        if !sessions.rename(session_id, title) {
            return None;
        }
        let cards = sessions.cards();
        let overrides = sessions.overrides().clone();
        drop(sessions);

        self.save_overrides(&overrides);
        Some(cards)
    }

    /// Puts a session away and remembers it. The transcript stays where it is:
    /// those files belong to Claude Code. tech.md 11.
    pub fn hide_session(&self, session_id: &str) -> Vec<SessionCard> {
        let mut sessions = self.lock(&self.sessions);
        sessions.hide(session_id);
        let cards = sessions.cards();
        let overrides = sessions.overrides().clone();
        drop(sessions);

        self.save_overrides(&overrides);
        cards
    }

    /// Loads what the user said about sessions, once, at startup.
    pub fn restore_sessions(&self) {
        let Some(path) = overrides_path() else {
            return;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return;
        };
        match serde_json::from_str::<SessionOverrides>(&raw) {
            Ok(overrides) => {
                tracing::debug!(
                    titles = overrides.titles.len(),
                    hidden = overrides.hidden.len(),
                    "restored what the user said about sessions"
                );
                self.lock(&self.sessions).restore(overrides);
            }
            // A broken file must not stop the overlay from starting. The user
            // loses their titles, not their agent.
            Err(err) => tracing::warn!(error = %err, "could not read sessions.json"),
        }
    }

    fn save_overrides(&self, overrides: &SessionOverrides) {
        let Some(path) = overrides_path() else {
            return;
        };
        let Ok(raw) = serde_json::to_string_pretty(overrides) else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(err) = std::fs::write(&path, raw) {
            tracing::warn!(error = %err, "could not write sessions.json");
        }
    }

    /// Puts sessions that stopped reporting back to rest. tech.md 6.3.
    pub fn rest_stale_work(&self, now: i64, after: i64) -> Option<Vec<SessionCard>> {
        let mut sessions = self.lock(&self.sessions);
        sessions
            .rest_stale_work(now, after)
            .then(|| sessions.cards().to_vec())
    }

    pub fn sessions(&self) -> Vec<SessionCard> {
        self.lock(&self.sessions).cards().to_vec()
    }

    /// Records one feed event and hands back the cards to broadcast.
    pub fn apply_feed(&self, event: FeedEvent, at: i64) -> Vec<SessionCard> {
        let mut registry = self.lock(&self.sessions);
        registry.apply(event, at);
        registry.cards().to_vec()
    }

    /// Opens the card if the session is new, then moves it to a status.
    /// A Stop can be the first event Peekle sees, so the card may not exist.
    pub fn set_session_status(
        &self,
        session: &SessionRef,
        status: SessionStatus,
        at: i64,
    ) -> Vec<SessionCard> {
        let mut registry = self.lock(&self.sessions);
        registry.ensure(session.clone(), at);
        registry.set_status(&session.session_id, status, at);
        registry.cards().to_vec()
    }

    /// Records what the agent said last. tech.md S6.
    /// What the user just sent, into the feed of the session it went to.
    pub fn user_turn(
        &self,
        session: &SessionRef,
        text: &str,
        state: peekle_core::types::EntryState,
        at: i64,
    ) -> Vec<SessionCard> {
        let mut sessions = self.lock(&self.sessions);
        sessions.user_turn(session.clone(), text, state, at);
        sessions.cards().to_vec()
    }

    /// The pty host. Commands spawn through it; the hook sink asks it whether
    /// an arriving session is one of ours.
    pub fn pty(&self) -> &peekle_core::pty::SharedPtyHost {
        &self.pty
    }

    /// Claims an id before the process behind it says anything, so the very
    /// first hook is already recognised as ours. tech.md 6.5.
    pub fn claim_session(&self, session_id: &str) {
        self.lock(&self.sessions).claim(session_id);
    }

    /// Opens the card for a session we just started, so the island has
    /// something to draw before the first hook lands.
    pub fn open_owned_session(&self, session: SessionRef, at: i64) -> Vec<SessionCard> {
        let mut sessions = self.lock(&self.sessions);
        sessions.open_owned(session, at);
        sessions.cards().to_vec()
    }

    /// Whether the island can type into this session.
    pub fn owns_session(&self, session_id: &str) -> bool {
        self.lock(&self.sessions).is_owned(session_id)
    }

    /// The process behind a session is gone. The card stays readable; only the
    /// claim goes, so the field knows there is nothing to type into.
    pub fn disown_session(&self, session_id: &str) {
        self.lock(&self.sessions).disown(session_id);
        self.pty.forget(session_id);
    }

    /// Marks the queued replies of a session as undeliverable. Only the path
    /// that could not start a turn calls this: a message that will never leave
    /// says so rather than sitting dim forever. tech.md 6.5.
    pub fn replies_failed(&self, session_id: &str, at: i64) -> Vec<SessionCard> {
        let mut sessions = self.lock(&self.sessions);
        sessions.replies_failed(session_id, at);
        sessions.cards().to_vec()
    }

    pub fn assistant_turn(&self, session: &SessionRef, text: &str, at: i64) -> Vec<SessionCard> {
        let mut registry = self.lock(&self.sessions);
        registry.assistant_turn(session.clone(), text, at);
        registry.cards().to_vec()
    }

    /// The session is over. Unknown sessions are left alone rather than being
    /// invented: Peekle may have started after the session did.
    pub fn mark_session_ended(&self, session_id: &str, at: i64) -> Vec<SessionCard> {
        let mut registry = self.lock(&self.sessions);
        registry.set_status(session_id, SessionStatus::Ended, at);
        registry.cards().to_vec()
    }

    /// The turn ended, so every row still `Running` never reported success.
    /// tech.md 6.3.
    pub fn end_turn(&self, session_id: &str, at: i64) -> Vec<SessionCard> {
        let mut registry = self.lock(&self.sessions);
        registry.end_turn(session_id, at);
        registry.cards().to_vec()
    }

    pub fn hotkey_ok(&self) -> bool {
        self.hotkey_ok.load(Ordering::Relaxed)
    }

    pub fn set_hotkey_ok(&self, value: bool) {
        self.hotkey_ok.store(value, Ordering::Relaxed);
    }

    pub fn prompt_timeout(&self) -> Duration {
        Duration::from_secs(self.lock_config().behavior.permission_wait_secs as u64)
    }

    pub fn active_prompt(&self) -> Option<PromptRequest> {
        self.lock(&self.active_prompt).clone()
    }

    /// Takes the prompt slot if it is free. A second blocking hook queues
    /// behind the first rather than replacing it.
    pub fn claim_prompt(&self, request: PromptRequest) -> bool {
        let mut active = self.lock(&self.active_prompt);
        if active.is_some() {
            self.lock(&self.queue).push(request);
            return false;
        }
        *active = Some(request);
        true
    }

    /// Clears the slot and hands back whatever was waiting behind it.
    pub fn release_prompt(&self, id: &str) -> Option<PromptRequest> {
        let mut active = self.lock(&self.active_prompt);
        if active.as_ref().is_some_and(|p| p.id != id) {
            return None;
        }
        *active = None;

        let mut queue = self.lock(&self.queue);
        if queue.is_empty() {
            return None;
        }
        let next = queue.remove(0);
        *active = Some(next.clone());
        Some(next)
    }

    pub fn tasks(&self) -> Vec<TaskItem> {
        self.lock(&self.tasks).clone()
    }

    /// Merges by id, keeps the freshest first and caps the list.
    pub fn merge_tasks(&self, incoming: Vec<TaskItem>) -> Vec<TaskItem> {
        let mut tasks = self.lock(&self.tasks);
        for item in incoming {
            match tasks.iter_mut().find(|t| t.id == item.id) {
                Some(existing) => *existing = item,
                None => tasks.push(item),
            }
        }
        tasks.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
        tasks.truncate(TASK_CAP);
        tasks.clone()
    }

    pub fn usage(&self) -> UsageSnapshot {
        self.lock(&self.usage).clone()
    }

    pub fn set_usage(&self, snapshot: UsageSnapshot) {
        *self.lock(&self.usage) = snapshot;
    }

    pub fn live_sessions(&self) -> u32 {
        self.live_sessions.load(Ordering::Relaxed)
    }

    pub fn session_started(&self) {
        self.live_sessions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn session_ended(&self) {
        let _ = self
            .live_sessions
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_sub(1))
            });
    }

    /// Handle a window waits on before being shown. tech.md section 8.
    pub fn ready_gate(&self, label: &str) -> Arc<Notify> {
        self.lock(&self.ready)
            .entry(label.to_string())
            .or_insert_with(|| Arc::new(Notify::new()))
            .clone()
    }

    pub fn snapshot(&self) -> PeekleState {
        PeekleState {
            enabled: self.enabled(),
            view: self.view(),
            active_prompt: self.active_prompt(),
            sessions: self.sessions(),
            tasks: self.tasks(),
            usage: self.usage(),
            live_sessions: self.live_sessions(),
            hotkey_ok: self.hotkey_ok(),
        }
    }

    pub fn lock_config(&self) -> std::sync::MutexGuard<'_, Config> {
        self.lock(&self.config)
    }

    /// Writes the config back. A failure is reported and swallowed: losing a
    /// remembered Keychain answer is worse than nothing, but not worth taking
    /// the overlay down for.
    pub fn save_config(&self) {
        let config = self.lock_config().clone();
        match peekle_core::config::config_path().and_then(|path| config.save(&path)) {
            Ok(()) => {}
            Err(err) => tracing::warn!(error = %err, "could not write the config"),
        }
    }

    /// A poisoned lock means an earlier holder panicked. The data is still
    /// sound, and refusing to answer would hang a live agent, so recover.
    fn lock<'a, T>(&self, target: &'a Mutex<T>) -> std::sync::MutexGuard<'a, T> {
        target.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peekle_core::types::{PromptKind, SessionRef, TaskLabel, TaskStatus};
    use peekle_usage::FakeUsage;

    /// S12. An island the user opened has to close itself, because Escape only
    /// reaches a panel that has already taken the keyboard.
    #[test]
    fn the_grace_period_starts_when_the_pointer_first_leaves() {
        let state = state();
        let now = Instant::now();
        let grace = Duration::from_millis(800);

        assert!(!state.pointer_left_for(grace, now), "the clock starts here");
        assert!(!state.pointer_left_for(grace, now + Duration::from_millis(799)));
        assert!(state.pointer_left_for(grace, now + grace));
    }

    #[test]
    fn coming_back_puts_the_grace_period_back_to_the_start() {
        let state = state();
        let now = Instant::now();
        let grace = Duration::from_millis(800);

        assert!(!state.pointer_left_for(grace, now));
        state.pointer_returned();

        let later = now + Duration::from_secs(10);
        assert!(!state.pointer_left_for(grace, later), "the clock restarted");
        assert!(state.pointer_left_for(grace, later + grace));
    }

    /// Only a crossing is worth an AppKit call, and a crossing back has to
    /// register too or the island keeps the mouse forever.
    #[test]
    fn the_mark_reports_a_crossing_and_only_a_crossing() {
        let state = state();

        assert!(state.set_over_rest(true));
        assert!(!state.set_over_rest(true));
        assert!(state.set_over_rest(false));
        assert!(!state.set_over_rest(false));
    }

    #[test]
    fn bounds_that_never_arrived_read_as_nothing_rather_than_zero() {
        let state = state();
        assert_eq!(state.shape_bounds(), None);

        state.set_shape_bounds((185.0, 47.0));
        assert_eq!(state.shape_bounds(), Some((185.0, 47.0)));
    }

    /// S12 follow up. Before the user grants access there is nothing to fetch,
    /// and telling them usage is off would send them looking for a switch that
    /// is not the problem.
    #[test]
    fn the_first_reason_names_what_is_actually_missing() {
        use peekle_core::config::{UsageConfig, UsageProviderKind};

        let account = |granted: bool, denied: bool| UsageConfig {
            enabled: true,
            provider: UsageProviderKind::Account,
            keychain_denied: denied,
            keychain_granted: granted,
        };

        assert_eq!(
            initial_reason(&account(false, false)),
            UsageUnavailable::NotGranted
        );
        assert_eq!(
            initial_reason(&account(false, true)),
            UsageUnavailable::Denied
        );
        assert_eq!(
            initial_reason(&account(true, false)),
            UsageUnavailable::Unsupported
        );

        let off = UsageConfig {
            enabled: false,
            ..account(true, false)
        };
        assert_eq!(initial_reason(&off), UsageUnavailable::Disabled);
    }

    /// v36. A request-opened island holds itself up for ten seconds, and
    /// engagement or expiry both end the hold exactly once.
    #[test]
    fn the_opening_hold_runs_out_and_clears_itself() {
        let state = state();
        let now = Instant::now();

        assert!(!state.held_open(now), "nothing held yet");
        state.hold_open(now + Duration::from_secs(10));
        assert!(state.held_open(now + Duration::from_secs(9)));
        assert!(!state.held_open(now + Duration::from_secs(10)), "expired");
        assert!(
            !state.held_open(now + Duration::from_secs(5)),
            "an expired hold cleared itself rather than coming back"
        );
    }

    #[test]
    fn engaging_ends_the_hold_early() {
        let state = state();
        let now = Instant::now();

        state.hold_open(now + Duration::from_secs(10));
        state.clear_hold();
        assert!(!state.held_open(now + Duration::from_secs(1)));
    }

    fn state() -> AppState {
        AppState::new(Config::default(), Arc::new(FakeUsage::default()))
    }

    fn request(id: &str) -> PromptRequest {
        PromptRequest {
            id: id.to_string(),
            kind: PromptKind::Permission,
            session: SessionRef {
                session_id: "s".into(),
                cwd: "/tmp".into(),
                project: "tmp".into(),
                pid: None,
                tty: None,
            },
            title: "t".into(),
            last_message: None,
            detail: None,
            options: Vec::new(),
            allow_free_text: true,
            created_at: 0,
            expires_at: 0,
        }
    }

    fn task(id: &str, updated_at: i64) -> TaskItem {
        TaskItem {
            id: id.to_string(),
            title: "work".into(),
            label: TaskLabel::Code,
            status: TaskStatus::Pending,
            session_id: "s".into(),
            updated_at,
        }
    }

    #[test]
    fn only_one_prompt_is_active_and_the_rest_queue() {
        let state = state();
        assert!(state.claim_prompt(request("a")));
        assert!(!state.claim_prompt(request("b")));
        assert_eq!(state.active_prompt().map(|p| p.id), Some("a".into()));
    }

    #[test]
    fn releasing_promotes_the_queued_request() {
        let state = state();
        state.claim_prompt(request("a"));
        state.claim_prompt(request("b"));

        let next = state.release_prompt("a");
        assert_eq!(next.map(|p| p.id), Some("b".into()));
        assert_eq!(state.active_prompt().map(|p| p.id), Some("b".into()));
    }

    #[test]
    fn releasing_a_stale_id_leaves_the_active_prompt_alone() {
        let state = state();
        state.claim_prompt(request("a"));
        assert!(state.release_prompt("gone").is_none());
        assert_eq!(state.active_prompt().map(|p| p.id), Some("a".into()));
    }

    #[test]
    fn tasks_merge_by_id_and_keep_the_freshest_first() {
        let state = state();
        state.merge_tasks(vec![task("1", 10), task("2", 20)]);
        let merged = state.merge_tasks(vec![task("1", 30)]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, "1");
        assert_eq!(merged[0].updated_at, 30);
    }

    #[test]
    fn task_list_is_capped() {
        let state = state();
        let many: Vec<_> = (0..80).map(|i| task(&i.to_string(), i)).collect();
        assert_eq!(state.merge_tasks(many).len(), TASK_CAP);
    }

    #[test]
    fn setting_the_same_view_twice_reports_no_move() {
        let state = state();
        assert_eq!(state.view(), IslandView::Collapsed);
        assert!(state.set_view(IslandView::Sessions));
        assert!(!state.set_view(IslandView::Sessions));
        assert!(state.set_view(IslandView::Session("s".into())));
        assert_eq!(state.view(), IslandView::Session("s".into()));
    }

    #[test]
    fn every_collapsed_view_lets_clicks_through_and_every_open_one_does_not() {
        assert!(!IslandView::Collapsed.takes_clicks());
        assert!(!IslandView::Pill.takes_clicks());
        assert!(IslandView::Sessions.takes_clicks());
        assert!(IslandView::Session("s".into()).takes_clicks());
    }

    #[test]
    fn live_session_count_never_goes_below_zero() {
        let state = state();
        state.session_ended();
        assert_eq!(state.live_sessions(), 0);
        state.session_started();
        state.session_started();
        state.session_ended();
        assert_eq!(state.live_sessions(), 1);
    }
}

/// Owning a session: who the island may type into. tech.md 6.5.
#[cfg(test)]
mod owned_tests {
    use super::*;
    use peekle_core::config::Config;
    use peekle_usage::FakeUsage;

    fn fresh() -> AppState {
        AppState::new(Config::default(), Arc::new(FakeUsage::default()))
    }

    /// A session Peekle did not start is observed, and observed means no
    /// input field. This is the replacement for the old ladder: instead of
    /// three ways to fail at typing into someone else's process, one honest no.
    #[test]
    fn a_session_it_never_started_is_not_owned() {
        let state = fresh();
        assert!(!state.owns_session("someone-elses"));
    }

    #[test]
    fn claiming_an_id_makes_the_session_ours() {
        let state = fresh();
        state.claim_session("ours");
        assert!(state.owns_session("ours"));
    }

    /// The claim goes when the process does, but the card does not: the
    /// conversation is still worth reading after the agent is gone.
    #[test]
    fn disowning_leaves_nothing_to_type_into() {
        let state = fresh();
        state.claim_session("ours");
        state.disown_session("ours");
        assert!(!state.owns_session("ours"));
    }

    #[test]
    fn ownership_is_per_session() {
        let state = fresh();
        state.claim_session("a");
        assert!(state.owns_session("a"));
        assert!(!state.owns_session("b"));
    }
}
