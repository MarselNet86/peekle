//! The update check, the question it raises and the way out of it. tech.md 6.30.
//!
//! Peekle has no dock icon and no menu bar, so there is nowhere for a person to
//! notice that a new version shipped. The app asks the repository itself, pulls
//! the file down in the background, and asks one question — with the file
//! already on disk, so the answer costs no waiting.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use peekle_core::types::{InstallKind, IslandView, Update, UpdateState};
use peekle_update::{GithubReleases, Updater};
use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::state::AppState;
use crate::windows;

/// How long after start the first check waits. tech.md 6.30.
///
/// The start is already busy backfilling transcripts and asking for usage, and
/// an update is the one thing here in no hurry whatsoever.
pub const FIRST_CHECK: Duration = Duration::from_secs(90);

/// The floor on the configured interval. A file that says `1` gets an hour, not
/// a minute: the key exists so the check can be slackened, not so it can be
/// turned into a poll.
pub const MIN_INTERVAL: Duration = Duration::from_secs(3600);

/// How long "Later" holds. A day: asked twice in one working day is nagging,
/// never asked again until a restart is a question nobody can get back to --
/// an accessory with no dock icon may run for weeks. tech.md 6.30.
pub const ASK_AGAIN_AFTER: Duration = Duration::from_secs(24 * 3600);

/// How often a put-off question looks at the clock. No network and no lock
/// held: a time compare and, once the day is up, the same raise a check does.
/// tech.md 6.30.
pub const ASK_AGAIN_POLL: Duration = Duration::from_secs(600);

/// Whether "Later", said at `at`, still holds at `now`.
///
/// Wall-clock time, not `Instant`: on macOS the monotonic clock stops while the
/// lid is closed, and a night asleep would not count toward the day. A clock
/// set back past the moment it was said keeps the promise rather than breaking
/// it early. tech.md 6.30.
pub fn still_put_off(at: SystemTime, now: SystemTime) -> bool {
    now.duration_since(at)
        .map(|since| since < ASK_AGAIN_AFTER)
        .unwrap_or(true)
}

/// What the app knows about updates right now.
///
/// Managed by Tauri beside `AppState` rather than folded into it: the whole of
/// it is two fields nobody else reads, and the state of a background check has
/// no business in the structure every hook path locks.
pub struct Updates {
    state: Mutex<UpdateState>,
    /// Versions the person said "Later" to, and when. The promise lasts
    /// `ASK_AGAIN_AFTER`, or until the process ends, whichever comes first.
    /// tech.md 6.30.
    dismissed: Mutex<Vec<(String, SystemTime)>>,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            state: Mutex::new(UpdateState::Idle),
            dismissed: Mutex::new(Vec::new()),
        }
    }
}

impl Updates {
    pub fn state(&self) -> UpdateState {
        self.state
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }

    /// The update on offer, if one is. `None` in every other state.
    pub fn ready(&self) -> Option<Update> {
        match self.state() {
            UpdateState::Ready(update) => Some(update),
            _ => None,
        }
    }

    fn set(&self, next: UpdateState) {
        if let Ok(mut held) = self.state.lock() {
            *held = next;
        }
    }

    fn was_dismissed(&self, version: &str) -> bool {
        self.was_dismissed_at(version, SystemTime::now())
    }

    fn was_dismissed_at(&self, version: &str, now: SystemTime) -> bool {
        self.dismissed
            .lock()
            .map(|held| {
                held.iter()
                    .any(|(seen, at)| seen == version && still_put_off(*at, now))
            })
            .unwrap_or(false)
    }

    fn dismiss(&self, version: &str) {
        self.dismiss_at(version, SystemTime::now());
    }

    /// A second "Later" moves the day along rather than keeping the first one:
    /// the person has just been asked and just answered.
    fn dismiss_at(&self, version: &str, at: SystemTime) {
        if let Ok(mut held) = self.dismissed.lock() {
            match held.iter_mut().find(|(seen, _)| seen == version) {
                Some(entry) => entry.1 = at,
                None => held.push((version.to_string(), at)),
            }
        }
    }
}

/// Whether a finished check should raise the question. tech.md 6.30.
///
/// Three things have to be true at once, and each is its own line of the
/// section: there is a file to install, the island is at rest, and this version
/// has not already been put off. An open island is never written over — the
/// person is reading a feed or answering a permission, and an update standing
/// on top of that is the pill the engaged-island rule forbids (6.7).
pub fn should_ask(state: &UpdateState, view: &IslandView, dismissed: bool) -> bool {
    state.asks() && matches!(view, IslandView::Collapsed) && !dismissed
}

/// The background check: once after `FIRST_CHECK`, then on the configured
/// interval. tech.md 6.30.
pub fn watch(app: &AppHandle, state: Arc<AppState>) {
    let (enabled, repo, hours) = {
        let config = state.lock_config();
        (
            config.update.enabled,
            config.update.repo.clone(),
            config.update.check_interval_hours,
        )
    };
    if !enabled || repo.trim().is_empty() {
        tracing::debug!("updates are not checked: no repository, or switched off");
        return;
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            let once = handle.clone();
            let repo = repo.clone();
            // Blocking: the ask and the download are synchronous, and a dmg
            // holds the thread for as long as it takes to arrive.
            let _ = tauri::async_runtime::spawn_blocking(move || check(&once, &repo)).await;

            // Zero means by hand only, so the first check was the only one.
            if hours == 0 {
                return;
            }
            tokio::time::sleep(MIN_INTERVAL.max(Duration::from_secs(hours as u64 * 3600))).await;
        }
    });
}

/// One check, start to finish. Answers the state it settled on. tech.md 6.30.
pub fn check(app: &AppHandle, repo: &str) -> UpdateState {
    let Some(updates) = app
        .try_state::<Arc<Updates>>()
        .map(|held| held.inner().clone())
    else {
        return UpdateState::Idle;
    };
    let updater = match build(app) {
        Some(updater) => updater,
        None => return UpdateState::Idle,
    };

    let announce = app.clone();
    let seen = Arc::clone(&updates);
    let settled = updater.check(repo, move |step| {
        seen.set(step.clone());
        emit(&announce, &step);
    });

    updates.set(settled.clone());
    emit(app, &settled);
    raise(app, &updates, &settled);
    settled
}

/// Builds the real updater around this build's version and its own bundle path.
fn build(app: &AppHandle) -> Option<Updater<GithubReleases>> {
    let version = app.package_info().version.to_string();
    let dir = peekle_update::cache_dir()?;
    // Where this copy sits decides how it updates: a bundle under a Caskroom
    // belongs to Homebrew. A path we cannot read is a plain bundle, which is
    // the answer that only ever offers a download the person still confirms.
    let bundle = std::env::current_exe()
        .ok()
        .unwrap_or_else(|| PathBuf::from("/Applications/Peekle.app"));
    Some(Updater::new(
        GithubReleases::new(&version),
        version,
        &bundle,
        dir,
    ))
}

fn emit(app: &AppHandle, state: &UpdateState) {
    if let Err(err) = app.emit(events::UPDATE, state) {
        tracing::warn!(error = %err, "failed to emit the update state");
    }
}

/// Raises the question, if this is the moment for it. tech.md 6.30.
fn raise(app: &AppHandle, updates: &Updates, state: &UpdateState) {
    let Some(update) = (match state {
        UpdateState::Ready(update) => Some(update),
        _ => None,
    }) else {
        return;
    };
    let Some(app_state) = app
        .try_state::<Arc<AppState>>()
        .map(|held| held.inner().clone())
    else {
        return;
    };
    if !should_ask(
        state,
        &app_state.view(),
        updates.was_dismissed(&update.version),
    ) {
        tracing::debug!(
            version = %update.version,
            "a version is ready and the island is not the place for it yet"
        );
        return;
    }
    tracing::info!(version = %update.version, install = ?update.install, "offering the update");
    windows::set_view(app, IslandView::Update);
}

/// "Later": the question goes away for a day, and comes back on its own once
/// the day is up and the island is at rest. tech.md 6.30.
pub fn dismiss(app: &AppHandle) {
    if let Some(updates) = app
        .try_state::<Arc<Updates>>()
        .map(|held| held.inner().clone())
    {
        if let Some(update) = updates.ready() {
            updates.dismiss(&update.version);
            tracing::debug!(version = %update.version, "the update was put off for a day");
            ask_again(app, updates, update.version);
        }
    }
    windows::set_view(app, IslandView::Collapsed);
}

/// Waits out the day and raises the question again, without waiting for the
/// next check. The check runs every `check_interval_hours`, six by default, and
/// a day that ends between two of them would otherwise run six hours long.
///
/// Ends as soon as there is nothing left to wait for: the offer changed or went
/// (installed, a newer version, a failed check), or the question is back up. A
/// second "Later" starts another of these, and the first one ends on its next
/// look because the version is put off again. tech.md 6.30.
fn ask_again(app: &AppHandle, updates: Arc<Updates>, version: String) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(ASK_AGAIN_POLL).await;
            let state = updates.state();
            let still_offered =
                matches!(&state, UpdateState::Ready(update) if update.version == version);
            if !still_offered {
                return;
            }
            if updates.was_dismissed(&version) {
                continue;
            }
            // `raise` asks the same three questions a check does, the resting
            // island among them; an open one is looked at again next time.
            raise(&handle, &updates, &state);
            let raised = handle
                .try_state::<Arc<AppState>>()
                .is_some_and(|app_state| app_state.view() == IslandView::Update);
            if raised {
                tracing::debug!(version = %version, "a day went by, asking about the update again");
                return;
            }
        }
    });
}

/// What "Install" does for a bundle: hand the dmg to Finder and get out of the
/// way. tech.md 6.30.
pub fn dmg_for(update: &Update) -> Option<PathBuf> {
    if update.install != InstallKind::Bundle {
        return None;
    }
    let dir = peekle_update::cache_dir()?;
    let file = peekle_update::dmg_path(&dir, &update.version);
    file.exists().then_some(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use peekle_core::types::UpdateError;

    fn ready(version: &str) -> UpdateState {
        UpdateState::Ready(Update {
            version: version.to_string(),
            notes_url: String::new(),
            size: 1,
            install: InstallKind::Bundle,
        })
    }

    /// The question needs a file, a resting island and a version nobody put
    /// off. Any one of the three missing keeps it down. tech.md 6.30.
    #[test]
    fn the_question_waits_for_rest_and_for_a_version_nobody_put_off() {
        assert!(should_ask(&ready("0.1.2"), &IslandView::Collapsed, false));

        assert!(
            !should_ask(&ready("0.1.2"), &IslandView::Collapsed, true),
            "a version already put off does not come back"
        );
        for view in [
            IslandView::Sessions,
            IslandView::Session("s".into()),
            IslandView::Ask,
            IslandView::Quit,
            IslandView::Bug,
            IslandView::Pill,
        ] {
            assert!(
                !should_ask(&ready("0.1.2"), &view, false),
                "an open island is never written over: {view:?}"
            );
        }
    }

    /// Every state but `Ready` raises nothing, failures included: nobody asked
    /// for the check, so nothing is reported to them. tech.md 6.30.
    #[test]
    fn nothing_but_a_ready_file_raises_the_island() {
        for state in [
            UpdateState::Idle,
            UpdateState::Checking,
            UpdateState::Downloading(Update {
                version: "0.1.2".into(),
                notes_url: String::new(),
                size: 1,
                install: InstallKind::Bundle,
            }),
            UpdateState::Failed(UpdateError::Offline),
            UpdateState::Failed(UpdateError::RateLimited),
            UpdateState::Failed(UpdateError::NoAsset),
            UpdateState::Failed(UpdateError::Download),
        ] {
            assert!(
                !should_ask(&state, &IslandView::Collapsed, false),
                "{state:?} has nothing to ask about"
            );
        }
    }

    /// A version put off stays put off for the day, and saying so twice does
    /// not double the list. tech.md 6.30.
    #[test]
    fn a_version_put_off_is_remembered_once() {
        let updates = Updates::default();
        assert!(!updates.was_dismissed("0.1.2"));
        updates.dismiss("0.1.2");
        updates.dismiss("0.1.2");
        assert!(updates.was_dismissed("0.1.2"));
        assert_eq!(
            updates.dismissed.lock().map(|held| held.len()).ok(),
            Some(1)
        );
        assert!(
            !updates.was_dismissed("0.1.3"),
            "the next version asks again"
        );
    }

    /// v87.3: "Later" holds for a day and not a second more. Owner: a question
    /// put off could not be got back to until the app was restarted.
    /// tech.md 6.30.
    #[test]
    fn later_holds_for_a_day_and_then_the_question_comes_back() {
        let said = SystemTime::UNIX_EPOCH + Duration::from_secs(1_789_000_000);
        let updates = Updates::default();
        updates.dismiss_at("0.1.2", said);

        let hour = Duration::from_secs(3600);
        assert!(updates.was_dismissed_at("0.1.2", said + hour));
        assert!(updates.was_dismissed_at("0.1.2", said + 23 * hour));
        assert!(
            !updates.was_dismissed_at("0.1.2", said + ASK_AGAIN_AFTER),
            "a day is up"
        );
        assert!(!updates.was_dismissed_at("0.1.2", said + 30 * hour));
    }

    /// A clock set back keeps the promise, and a second "Later" moves the day
    /// along from the moment it was said. tech.md 6.30.
    #[test]
    fn a_clock_set_back_keeps_the_promise_and_a_second_later_moves_it() {
        let said = SystemTime::UNIX_EPOCH + Duration::from_secs(1_789_000_000);
        let hour = Duration::from_secs(3600);
        assert!(still_put_off(said, said - hour), "the clock went back");

        let updates = Updates::default();
        updates.dismiss_at("0.1.2", said);
        updates.dismiss_at("0.1.2", said + 20 * hour);
        assert!(
            updates.was_dismissed_at("0.1.2", said + 30 * hour),
            "the second Later started its own day"
        );
        assert!(!updates.was_dismissed_at("0.1.2", said + 44 * hour));
    }

    /// Homebrew never has a file to open: the offer is the version and the
    /// command, and `brew upgrade` is what installs it. tech.md 6.30.
    #[test]
    fn a_homebrew_update_has_no_dmg_to_open() {
        let brewed = Update {
            version: "0.1.2".into(),
            notes_url: String::new(),
            size: 0,
            install: InstallKind::Homebrew,
        };
        assert!(dmg_for(&brewed).is_none());
    }
}
