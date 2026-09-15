//! The fake behind `Releases`, so dev and the tests never touch the network.
//! tech.md section 7.

use std::path::Path;
use std::sync::Mutex;

use peekle_core::types::UpdateError;

use crate::{Asset, Release, Releases};

/// A scripted repository: one answer for the ask, one for the download.
///
/// Both are set by the test rather than discovered, and the bytes it writes are
/// as long as the asset says it is, so the size check in `Updater::pull` has
/// something real to pass or fail against.
pub struct FakeReleases {
    latest: Mutex<Result<Release, UpdateError>>,
    /// What `download` does. `Ok(n)` writes `n` bytes, whatever the asset says,
    /// which is how a truncated download is staged.
    download: Mutex<Result<u64, UpdateError>>,
    asks: Mutex<u32>,
    pulls: Mutex<u32>,
}

impl FakeReleases {
    /// A repository whose latest release is `release`, downloading whole.
    pub fn with(release: Release) -> Self {
        Self {
            latest: Mutex::new(Ok(release)),
            download: Mutex::new(Ok(u64::MAX)),
            asks: Mutex::new(0),
            pulls: Mutex::new(0),
        }
    }

    /// A repository that answers the ask with `reason`.
    pub fn failing(reason: UpdateError) -> Self {
        Self {
            latest: Mutex::new(Err(reason)),
            download: Mutex::new(Ok(u64::MAX)),
            asks: Mutex::new(0),
            pulls: Mutex::new(0),
        }
    }

    /// Writes `bytes` on every download instead of the asset's own size, which
    /// is how a short file is staged.
    pub fn writing(self, bytes: u64) -> Self {
        *self.download.lock().expect("download lock") = Ok(bytes);
        self
    }

    /// How many times the release was asked for, so a test can prove a second
    /// check reused the file it already had.
    pub fn asks(&self) -> u32 {
        *self.asks.lock().expect("asks lock")
    }

    /// How many times a file was actually pulled.
    pub fn pulls(&self) -> u32 {
        *self.pulls.lock().expect("pulls lock")
    }
}

impl Releases for FakeReleases {
    fn latest(&self, _repo: &str) -> Result<Release, UpdateError> {
        *self.asks.lock().expect("asks lock") += 1;
        self.latest.lock().expect("latest lock").clone()
    }

    fn download(&self, asset: &Asset, into: &Path) -> Result<u64, UpdateError> {
        *self.pulls.lock().expect("pulls lock") += 1;
        let planned = *self.download.lock().expect("download lock");
        let bytes = match planned {
            Ok(u64::MAX) => asset.size,
            Ok(bytes) => bytes,
            Err(reason) => return Err(reason),
        };
        if let Some(parent) = into.parent() {
            std::fs::create_dir_all(parent).map_err(|_| UpdateError::Download)?;
        }
        std::fs::write(into, vec![0u8; bytes as usize]).map_err(|_| UpdateError::Download)?;
        Ok(bytes)
    }
}
