//! New versions, from the repository's own releases. tech.md sections 6.30 and 7.
//!
//! Nothing here installs anything. The crate answers one question — is there a
//! release newer than this build, and is its dmg on disk — and the last step
//! stays with the person in Finder. The reason is in 6.30: the builds carry an
//! ad-hoc signature (R-7) and there is no key to verify a download with, so an
//! app that silently replaced itself with a file from the network would be an
//! installer for whatever the configured address served.

pub mod fake;
pub mod github;

pub use fake::FakeReleases;
pub use github::GithubReleases;

use std::path::{Path, PathBuf};

use peekle_core::types::{InstallKind, Update, UpdateError, UpdateState};

/// The asset a macOS release is expected to carry. The name holds no version
/// on purpose, so `releases/latest/download/<name>` outlives every tag.
/// tech.md 6.27.
pub const DMG_NAME: &str = "Peekle-mac-universal.dmg";

/// What Homebrew would run to do this itself. Copied to the pasteboard rather
/// than executed: brew owns the bundle and this is brew's job. tech.md 6.30.
pub const BREW_COMMAND: &str = "brew upgrade --cask peekle";

/// One release of the repository, as GitHub describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The tag as published, `v` and all.
    pub tag: String,
    /// The release page, for whoever would rather read before installing.
    pub html_url: String,
    pub assets: Vec<Asset>,
}

/// One file attached to a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    /// Bytes, as the release names them. Zero when it names none.
    pub size: u64,
}

impl Release {
    /// The macOS dmg of this release, if it carries one.
    pub fn dmg(&self) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == DMG_NAME)
    }
}

/// Where releases are read from. tech.md section 7.
pub trait Releases: Send + Sync + 'static {
    /// The newest published release of `repo`, as `owner/name`.
    fn latest(&self, repo: &str) -> Result<Release, UpdateError>;

    /// Pulls `asset` to `into`, whole or not at all. Answers the bytes written.
    fn download(&self, asset: &Asset, into: &Path) -> Result<u64, UpdateError>;
}

/// Three numbers, which is all a tag is allowed to be. tech.md 6.30.
///
/// Not a string comparison: `0.1.10` sorts below `0.1.9` as text, and a build
/// that got this wrong would stop offering updates exactly when the tenth
/// patch shipped. A tag that does not part into three numbers is not a version
/// at all, and a build that cannot read a tag has to stay quiet rather than
/// offer to install something it did not understand.
pub fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let trimmed = raw.trim();
    let digits = trimmed.strip_prefix('v').unwrap_or(trimmed);
    // A tag may carry a suffix (`-rc1`, `+build`); the numbers end where it
    // starts, and what follows never makes the release newer.
    let numbers = digits
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .collect::<Vec<_>>();
    if numbers.len() != 3 {
        return None;
    }
    let mut parts = [0u64; 3];
    for (slot, text) in parts.iter_mut().zip(numbers) {
        *slot = text.parse().ok()?;
    }
    Some((parts[0], parts[1], parts[2]))
}

/// The version `tag` names, if it is newer than `current`. tech.md 6.30.
///
/// `None` covers every reason not to offer anything: the same version, an
/// older one, a tag nobody can read, and a build whose own version is
/// unreadable — that last one matters, because a build that cannot say what it
/// is cannot say that anything is newer than it.
pub fn newer_than(tag: &str, current: &str) -> Option<String> {
    let theirs = parse_version(tag)?;
    let ours = parse_version(current)?;
    if theirs > ours {
        let trimmed = tag.trim();
        Some(trimmed.strip_prefix('v').unwrap_or(trimmed).to_string())
    } else {
        None
    }
}

/// How this copy of Peekle was installed, read from where it sits. tech.md 6.30.
///
/// The path is enough and asking `brew` is not needed: a cask bundle lives
/// under a Caskroom, and running a subprocess to learn what the path already
/// says would cost a process on every check.
pub fn install_kind(bundle: &Path) -> InstallKind {
    let path = bundle.to_string_lossy();
    if path.contains("/Caskroom/") {
        InstallKind::Homebrew
    } else {
        InstallKind::Bundle
    }
}

/// Where downloaded dmgs wait. A cache, in the cache directory: losing it
/// costs one download. tech.md 6.30.
pub fn cache_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.cache_dir().join("peekle").join("updates"))
}

/// The dmg of one version, and the half-written file it arrives as. Two names
/// rather than one: a half dmg under the final name is worse than no file,
/// because the next run would take it for a finished download. tech.md 6.30.
pub fn dmg_path(dir: &Path, version: &str) -> PathBuf {
    dir.join(format!("{version}.dmg"))
}

pub fn part_path(dir: &Path, version: &str) -> PathBuf {
    dir.join(format!("{version}.dmg.part"))
}

/// Drops every file in the cache but the one version still on offer. A dmg is
/// a hundred megabytes and there is no reason to keep a pile of them.
pub fn sweep(dir: &Path, keep: &str) -> usize {
    let keep_dmg = dmg_path(dir, keep);
    let keep_part = part_path(dir, keep);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut dropped = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep_dmg || path == keep_part {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            dropped += 1;
        }
    }
    dropped
}

/// The check, end to end: ask, compare, and pull the file down if there is one.
/// tech.md 6.30.
///
/// Every step answers with an `UpdateState` rather than an error, because none
/// of this reaches the screen on its own: a failed check is a line in the log
/// and a reason to try again later, never a banner over somebody's work.
pub struct Updater<R: Releases> {
    releases: R,
    /// This build's own version, `CARGO_PKG_VERSION` in the app.
    current: String,
    /// Where this copy of Peekle sits, which decides how it updates.
    install: InstallKind,
    dir: PathBuf,
}

impl<R: Releases> Updater<R> {
    pub fn new(releases: R, current: impl Into<String>, bundle: &Path, dir: PathBuf) -> Self {
        Self {
            releases,
            current: current.into(),
            install: install_kind(bundle),
            dir,
        }
    }

    pub fn install_kind(&self) -> InstallKind {
        self.install
    }

    /// The repository behind this updater, so a test can ask how many times it
    /// was actually reached.
    pub fn releases(&self) -> &R {
        &self.releases
    }

    /// Asks the repository what it has, and reports what should happen next.
    ///
    /// `report` is called with every state on the way — `Downloading` before a
    /// long pull, so the app can tell one from a check that simply hung — and
    /// the final state is also the answer.
    pub fn check(&self, repo: &str, mut report: impl FnMut(UpdateState)) -> UpdateState {
        if repo.trim().is_empty() {
            // No address is not a failure: nobody set one, so nothing is asked.
            return UpdateState::Idle;
        }
        report(UpdateState::Checking);

        let release = match self.releases.latest(repo) {
            Ok(release) => release,
            Err(reason) => {
                tracing::warn!(?reason, repo, "could not ask for the latest release");
                return UpdateState::Failed(reason);
            }
        };

        let Some(version) = newer_than(&release.tag, &self.current) else {
            tracing::debug!(tag = %release.tag, current = %self.current, "this build is current");
            return UpdateState::Idle;
        };

        // Homebrew stops here on purpose: the bundle belongs to brew, and a dmg
        // dragged over it leaves brew's receipt on the old version, so the next
        // `brew upgrade` walks the person back. tech.md 6.30.
        if self.install == InstallKind::Homebrew {
            return UpdateState::Ready(Update {
                version,
                notes_url: release.html_url,
                size: 0,
                install: InstallKind::Homebrew,
            });
        }

        let Some(asset) = release.dmg() else {
            tracing::warn!(tag = %release.tag, "the release carries no {DMG_NAME}");
            return UpdateState::Failed(UpdateError::NoAsset);
        };

        let update = Update {
            version: version.clone(),
            notes_url: release.html_url.clone(),
            size: asset.size,
            install: InstallKind::Bundle,
        };

        let whole = dmg_path(&self.dir, &version);
        // A file already here is this version, downloaded whole on an earlier
        // run: the name carries the version and only a finished file gets it.
        if whole.exists() {
            sweep(&self.dir, &version);
            return UpdateState::Ready(update);
        }

        report(UpdateState::Downloading(update.clone()));
        match self.pull(asset, &version) {
            Ok(()) => {
                sweep(&self.dir, &version);
                UpdateState::Ready(update)
            }
            Err(reason) => {
                tracing::warn!(?reason, version, "the update did not come down");
                UpdateState::Failed(reason)
            }
        }
    }

    /// Writes the dmg under `.part` and renames it only once it is whole and
    /// the right size.
    fn pull(&self, asset: &Asset, version: &str) -> Result<(), UpdateError> {
        std::fs::create_dir_all(&self.dir).map_err(|err| {
            tracing::warn!(error = %err, "no cache directory for updates");
            UpdateError::Download
        })?;
        let part = part_path(&self.dir, version);
        let written = self.releases.download(asset, &part)?;

        // Zero means the release named no size, and then there is nothing to
        // check against; a named size that does not match is a truncated file.
        if asset.size != 0 && written != asset.size {
            let _ = std::fs::remove_file(&part);
            tracing::warn!(written, expected = asset.size, "the dmg came down short");
            return Err(UpdateError::Download);
        }

        std::fs::rename(&part, dmg_path(&self.dir, version)).map_err(|err| {
            let _ = std::fs::remove_file(&part);
            tracing::warn!(error = %err, "could not put the dmg under its final name");
            UpdateError::Download
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_parts_into_three_numbers_and_nothing_else_does() {
        assert_eq!(parse_version("v0.1.2"), Some((0, 1, 2)));
        assert_eq!(parse_version("0.1.2"), Some((0, 1, 2)));
        assert_eq!(parse_version(" v1.20.300 "), Some((1, 20, 300)));
        assert_eq!(parse_version("v0.1.2-rc1"), Some((0, 1, 2)));

        assert_eq!(parse_version("v0.1"), None);
        assert_eq!(parse_version("0.1.2.3"), None);
        assert_eq!(parse_version("nightly"), None);
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("v0.1.x"), None);
    }

    /// The one comparison a string would get wrong, and the reason this is not
    /// a string comparison at all.
    #[test]
    fn the_tenth_patch_is_newer_than_the_ninth() {
        assert_eq!(newer_than("v0.1.10", "0.1.9"), Some("0.1.10".to_string()));
        assert_eq!(newer_than("v0.1.9", "0.1.10"), None);
    }

    #[test]
    fn only_a_higher_version_is_offered() {
        assert_eq!(newer_than("v0.1.2", "0.1.1"), Some("0.1.2".to_string()));
        assert_eq!(newer_than("v0.2.0", "0.1.99"), Some("0.2.0".to_string()));
        assert_eq!(newer_than("v0.1.1", "0.1.1"), None, "the same is not newer");
        assert_eq!(newer_than("v0.1.0", "0.1.1"), None);
    }

    /// A tag nobody can read offers nothing, and neither does a build that
    /// cannot say what it is.
    #[test]
    fn an_unreadable_version_on_either_side_offers_nothing() {
        assert_eq!(newer_than("nightly", "0.1.1"), None);
        assert_eq!(newer_than("v9.9.9", "whatever"), None);
    }

    #[test]
    fn a_bundle_under_a_caskroom_belongs_to_homebrew() {
        assert_eq!(
            install_kind(Path::new("/opt/homebrew/Caskroom/peekle/0.1.1/Peekle.app")),
            InstallKind::Homebrew
        );
        assert_eq!(
            install_kind(Path::new("/usr/local/Caskroom/peekle/0.1.1/Peekle.app")),
            InstallKind::Homebrew
        );
        assert_eq!(
            install_kind(Path::new("/Applications/Peekle.app")),
            InstallKind::Bundle
        );
        assert_eq!(
            install_kind(Path::new("/Users/someone/Downloads/Peekle.app")),
            InstallKind::Bundle
        );
    }

    #[test]
    fn a_release_without_the_mac_dmg_has_no_dmg() {
        let release = Release {
            tag: "v0.1.2".into(),
            html_url: "https://example.invalid/r".into(),
            assets: vec![Asset {
                name: "Peekle-win-x64.msi".into(),
                url: "https://example.invalid/a".into(),
                size: 10,
            }],
        };
        assert!(release.dmg().is_none());
    }
}
