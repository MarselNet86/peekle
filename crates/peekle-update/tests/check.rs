//! The check, end to end, against the fake repository. tech.md 6.30.
//!
//! Every case here is one line of the section: what is offered, what is not,
//! what is downloaded twice, and what every failure does instead of reaching
//! the screen.

use std::path::{Path, PathBuf};

use peekle_core::types::{InstallKind, UpdateError, UpdateState};
use peekle_update::{dmg_path, part_path, Asset, FakeReleases, Release, Updater, DMG_NAME};

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("peekle-update-{}", ulid::Ulid::generate()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn release(tag: &str, size: u64) -> Release {
    Release {
        tag: tag.to_string(),
        html_url: format!("https://github.com/MarselNet86/peekle/releases/tag/{tag}"),
        assets: vec![Asset {
            name: DMG_NAME.to_string(),
            url: "https://example.invalid/Peekle-mac-universal.dmg".to_string(),
            size,
        }],
    }
}

fn bundle() -> &'static Path {
    Path::new("/Applications/Peekle.app")
}

/// Collects every state the check passed through, so a test can say what the
/// island would have seen and in what order.
fn run(updater: &Updater<FakeReleases>, repo: &str) -> (Vec<UpdateState>, UpdateState) {
    let mut seen = Vec::new();
    let end = updater.check(repo, |state| seen.push(state));
    (seen, end)
}

/// A newer tag is pulled down whole and offered, and the island hears about the
/// download before it hears about the question.
#[test]
fn a_newer_release_comes_down_and_is_offered() {
    let dir = scratch();
    let updater = Updater::new(
        FakeReleases::with(release("v0.1.2", 2048)),
        "0.1.1",
        bundle(),
        dir.clone(),
    );

    let (seen, end) = run(&updater, "MarselNet86/peekle");

    assert!(
        matches!(seen.first(), Some(UpdateState::Checking)),
        "the check says so before it asks: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|state| matches!(state, UpdateState::Downloading(_))),
        "a long pull is announced, so a hung check is not mistaken for it: {seen:?}"
    );

    let UpdateState::Ready(update) = end else {
        panic!("a newer release is offered, got {end:?}");
    };
    assert_eq!(update.version, "0.1.2", "the tag is shown without its v");
    assert_eq!(update.size, 2048);
    assert_eq!(update.install, InstallKind::Bundle);
    assert!(update.notes_url.contains("v0.1.2"));

    let file = dmg_path(&dir, "0.1.2");
    assert!(file.exists(), "the file is on disk before the question");
    assert!(
        !part_path(&dir, "0.1.2").exists(),
        "nothing is left under the part name"
    );
}

/// The build is current, or the release is older. Nothing is asked, nothing is
/// downloaded, and `Idle` is not a failure.
#[test]
fn the_same_or_an_older_release_is_not_offered() {
    let dir = scratch();
    for tag in ["v0.1.1", "v0.1.0"] {
        let updater = Updater::new(
            FakeReleases::with(release(tag, 2048)),
            "0.1.1",
            bundle(),
            dir.clone(),
        );
        let (_, end) = run(&updater, "MarselNet86/peekle");
        assert_eq!(end, UpdateState::Idle, "{tag} should not be offered");
    }
    assert!(
        std::fs::read_dir(&dir).expect("cache").next().is_none(),
        "nothing is downloaded for a release that is not newer"
    );
}

/// An empty `update.repo` is the off switch: there is no address to ask, so
/// nothing is asked, and that is not a failure either.
#[test]
fn an_empty_repo_asks_nobody() {
    let dir = scratch();
    let updater = Updater::new(
        FakeReleases::with(release("v9.9.9", 2048)),
        "0.1.1",
        bundle(),
        dir,
    );

    let (seen, end) = run(&updater, "   ");

    assert_eq!(end, UpdateState::Idle);
    assert!(seen.is_empty(), "not even Checking: {seen:?}");
    assert_eq!(updater.releases().asks(), 0, "github is never reached");
}

/// A copy installed by Homebrew is offered the version and never the file:
/// brew owns the bundle, and a dmg dragged over it leaves brew's receipt on the
/// old version.
#[test]
fn a_homebrew_copy_is_told_about_the_version_and_downloads_nothing() {
    let dir = scratch();
    let releases = FakeReleases::with(release("v0.1.2", 2048));
    let updater = Updater::new(
        releases,
        "0.1.1",
        Path::new("/opt/homebrew/Caskroom/peekle/0.1.1/Peekle.app"),
        dir.clone(),
    );

    let (seen, end) = run(&updater, "MarselNet86/peekle");

    let UpdateState::Ready(update) = end else {
        panic!("the version is still offered, got {end:?}");
    };
    assert_eq!(update.install, InstallKind::Homebrew);
    assert_eq!(update.size, 0, "there is no file, so there is no size");
    assert!(
        !seen
            .iter()
            .any(|state| matches!(state, UpdateState::Downloading(_))),
        "nothing is pulled for a cask: {seen:?}"
    );
    assert!(
        std::fs::read_dir(&dir).expect("cache").next().is_none(),
        "the cache stays empty"
    );
}

/// The second check finds the file it wrote and does not pull it again.
#[test]
fn a_version_already_on_disk_is_not_downloaded_twice() {
    let dir = scratch();
    let updater = Updater::new(
        FakeReleases::with(release("v0.1.2", 2048)),
        "0.1.1",
        bundle(),
        dir.clone(),
    );

    let first = updater.check("MarselNet86/peekle", |_| {});
    let second = updater.check("MarselNet86/peekle", |_| {});

    assert!(matches!(first, UpdateState::Ready(_)));
    assert_eq!(first, second, "the same answer both times");
    assert_eq!(
        updater.releases().pulls(),
        1,
        "the file is pulled once and reused"
    );
}

/// A dmg that arrives shorter than the release said is thrown away rather than
/// offered: half a dmg under the finished name would be taken for a download.
#[test]
fn a_short_download_is_thrown_away_rather_than_offered() {
    let dir = scratch();
    let updater = Updater::new(
        FakeReleases::with(release("v0.1.2", 2048)).writing(900),
        "0.1.1",
        bundle(),
        dir.clone(),
    );

    let (_, end) = run(&updater, "MarselNet86/peekle");

    assert_eq!(end, UpdateState::Failed(UpdateError::Download));
    assert!(!dmg_path(&dir, "0.1.2").exists(), "no finished file");
    assert!(!part_path(&dir, "0.1.2").exists(), "and no half one either");
}

/// Every way the ask can fail ends as `Failed`, which raises nothing. The next
/// check tries again.
#[test]
fn a_failed_ask_offers_nothing_and_raises_nothing() {
    for reason in [
        UpdateError::Offline,
        UpdateError::RateLimited,
        UpdateError::NoAsset,
    ] {
        let updater = Updater::new(FakeReleases::failing(reason), "0.1.1", bundle(), scratch());
        let (_, end) = run(&updater, "MarselNet86/peekle");
        assert_eq!(end, UpdateState::Failed(reason));
        assert!(!end.asks(), "a failure never raises the question");
    }
}

/// A release that carries no macOS dmg is a release this platform cannot use.
#[test]
fn a_release_without_the_dmg_is_not_offered() {
    let dir = scratch();
    let bare = Release {
        tag: "v0.1.2".into(),
        html_url: "https://example.invalid/r".into(),
        assets: vec![Asset {
            name: "Peekle-win-x64.msi".into(),
            url: "https://example.invalid/a".into(),
            size: 10,
        }],
    };
    let updater = Updater::new(FakeReleases::with(bare), "0.1.1", bundle(), dir);

    let (_, end) = run(&updater, "MarselNet86/peekle");

    assert_eq!(end, UpdateState::Failed(UpdateError::NoAsset));
}

/// The cache holds the version on offer and nothing else: a dmg is a hundred
/// megabytes and there is no reason to keep a pile of them.
#[test]
fn the_cache_keeps_only_the_version_on_offer() {
    let dir = scratch();
    std::fs::write(dmg_path(&dir, "0.0.9"), b"old").expect("stale dmg");
    std::fs::write(part_path(&dir, "0.0.8"), b"older").expect("stale part");

    let updater = Updater::new(
        FakeReleases::with(release("v0.1.2", 2048)),
        "0.1.1",
        bundle(),
        dir.clone(),
    );
    let (_, end) = run(&updater, "MarselNet86/peekle");

    assert!(matches!(end, UpdateState::Ready(_)));
    let left: Vec<_> = std::fs::read_dir(&dir)
        .expect("cache")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(left, vec!["0.1.2.dmg".to_string()], "only the one on offer");
}

/// `Ready` is the one state that raises the question, and the island leans on
/// exactly that.
#[test]
fn only_ready_asks() {
    assert!(!UpdateState::Idle.asks());
    assert!(!UpdateState::Checking.asks());
    assert!(!UpdateState::Failed(UpdateError::Offline).asks());
    assert!(UpdateState::Ready(peekle_core::types::Update {
        version: "0.1.2".into(),
        notes_url: String::new(),
        size: 0,
        install: InstallKind::Bundle,
    })
    .asks());
}
