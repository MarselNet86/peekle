//! The update check, the question it raises and the way out of it. tech.md 6.30.
//!
//! Peekle has no dock icon and no menu bar, so there is nowhere for a person to
//! notice that a new version shipped. The app asks the repository itself, pulls
//! the file down in the background, and asks one question — with the file
//! already on disk, so the answer costs no waiting.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

/// What the app knows about updates right now.
///
/// Managed by Tauri beside `AppState` rather than folded into it: the whole of
/// it is two fields nobody else reads, and the state of a background check has
/// no business in the structure every hook path locks.
pub struct Updates {
    state: Mutex<UpdateState>,
    /// Versions the person said "Later" to. Held for the life of the process,
    /// which is exactly how long the promise lasts: a question asked twice in
    /// one working day is not care, it is nagging. tech.md 6.30.
    dismissed: Mutex<Vec<String>>,
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
        self.dismissed
            .lock()
            .map(|held| held.iter().any(|seen| seen == version))
            .unwrap_or(false)
    }

    fn dismiss(&self, version: &str) {
        if let Ok(mut held) = self.dismissed.lock() {
            if !held.iter().any(|seen| seen == version) {
                held.push(version.to_string());
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

/// "Later": the question goes away and this version does not come back until
/// the app is restarted. tech.md 6.30.
pub fn dismiss(app: &AppHandle) {
    if let Some(updates) = app.try_state::<Arc<Updates>>() {
        if let Some(update) = updates.ready() {
            updates.dismiss(&update.version);
            tracing::debug!(version = %update.version, "the update was put off");
        }
    }
    windows::set_view(app, IslandView::Collapsed);
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

    /// A version put off once stays put off, and saying so twice does not
    /// double the list.
    #[test]
    fn a_version_put_off_is_remembered_once() {
        let updates = Updates::default();
        assert!(!updates.was_dismissed("0.1.2"));
        updates.dismiss("0.1.2");
        updates.dismiss("0.1.2");
        assert!(updates.was_dismissed("0.1.2"));
        assert!(
            !updates.was_dismissed("0.1.3"),
            "the next version asks again"
        );
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
