//! The one owner of application state. The frontend renders `PeekleState` and
//! sends intents; it holds no authoritative state of its own. tech.md 6.3, 8.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use peekle_core::config::Config;
use peekle_core::types::{PeekleState, PromptRequest, TaskItem, UsageSnapshot, UsageUnavailable};
use peekle_core::PendingRegistry;
use peekle_usage::{fake::unknown, UsageProvider};
use tokio::sync::Notify;

/// Freshest activity first, capped. tech.md 6.3.
const TASK_CAP: usize = 50;

pub struct AppState {
    pub config: Mutex<Config>,
    pub pending: PendingRegistry,
    pub usage_provider: Arc<dyn UsageProvider>,

    enabled: AtomicBool,
    hotkey_ok: AtomicBool,
    live_sessions: AtomicU32,
    active_prompt: Mutex<Option<PromptRequest>>,
    /// Blocking requests that arrived while a prompt was already open. Not part
    /// of `PeekleState`: the frontend never sees the queue. tech.md 6.3.
    queue: Mutex<Vec<PromptRequest>>,
    tasks: Mutex<Vec<TaskItem>>,
    usage: Mutex<UsageSnapshot>,
    ready: Mutex<HashMap<String, Arc<Notify>>>,
}

impl AppState {
    pub fn new(config: Config, usage_provider: Arc<dyn UsageProvider>) -> Self {
        let enabled = config.behavior.enabled;
        Self {
            config: Mutex::new(config),
            pending: PendingRegistry::new(),
            usage_provider,
            enabled: AtomicBool::new(enabled),
            hotkey_ok: AtomicBool::new(true),
            live_sessions: AtomicU32::new(0),
            active_prompt: Mutex::new(None),
            queue: Mutex::new(Vec::new()),
            tasks: Mutex::new(Vec::new()),
            usage: Mutex::new(unknown(UsageUnavailable::Disabled)),
            ready: Mutex::new(HashMap::new()),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, value: bool) {
        self.enabled.store(value, Ordering::Relaxed);
    }

    pub fn hotkey_ok(&self) -> bool {
        self.hotkey_ok.load(Ordering::Relaxed)
    }

    pub fn prompt_timeout(&self) -> Duration {
        Duration::from_secs(self.lock_config().behavior.prompt_timeout_secs as u64)
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
            active_prompt: self.active_prompt(),
            tasks: self.tasks(),
            usage: self.usage(),
            live_sessions: self.live_sessions(),
            hotkey_ok: self.hotkey_ok(),
        }
    }

    pub fn lock_config(&self) -> std::sync::MutexGuard<'_, Config> {
        self.lock(&self.config)
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

    fn state() -> AppState {
        AppState::new(Config::default(), Arc::new(FakeUsage::default()))
    }

    fn request(id: &str) -> PromptRequest {
        PromptRequest {
            id: id.to_string(),
            kind: PromptKind::Stop,
            session: SessionRef {
                session_id: "s".into(),
                cwd: "/tmp".into(),
                project: "tmp".into(),
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
