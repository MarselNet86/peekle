//! `Releases` against api.github.com. tech.md 6.30.
//!
//! Anonymous, and it stays anonymous: the releases of a public repository are
//! public, and keeping a token to read what is already open would be a secret
//! kept for nothing (rule 11). GitHub allows an anonymous caller 60 requests an
//! hour, and a check every six hours spends four a day.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use peekle_core::types::UpdateError;
use serde::Deserialize;

use crate::{Asset, Release, Releases};

/// How long the ask may take. A check nobody started must not sit on a socket.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(15);

/// How long the dmg may take. Generous: it is a hundred megabytes and it runs
/// in the background, where nobody is waiting for it.
pub const PULL_TIMEOUT: Duration = Duration::from_secs(600);

/// Refuses a body larger than a Peekle dmg could plausibly be. A server that
/// answers with something else does not get to fill the disk.
const MAX_BYTES: u64 = 512 * 1024 * 1024;

pub struct GithubReleases {
    /// `peekle/<version>`, which GitHub asks every caller to send.
    agent: String,
}

impl GithubReleases {
    pub fn new(version: &str) -> Self {
        Self {
            agent: format!("peekle/{version}"),
        }
    }
}

/// Only the fields that are used. GitHub sends many more and sends new ones
/// over time; naming four keeps this from breaking when it does.
#[derive(Debug, Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Debug, Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

impl Releases for GithubReleases {
    fn latest(&self, repo: &str) -> Result<Release, UpdateError> {
        let url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let response = ureq::get(&url)
            .config()
            .timeout_global(Some(ASK_TIMEOUT))
            // A refusal is an answer: 403 means the hour's requests are spent,
            // and turning it into a bare error throws that apart from offline.
            .http_status_as_error(false)
            .build()
            .header("user-agent", &self.agent)
            .header("accept", "application/vnd.github+json")
            .call();

        let mut response = match response {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!(error = %err, repo, "could not reach github");
                return Err(UpdateError::Offline);
            }
        };

        match response.status().as_u16() {
            200 => {}
            // 403 is the anonymous rate limit, 429 the explicit one.
            403 | 429 => return Err(UpdateError::RateLimited),
            // A repository with no published release yet answers 404, and that
            // is not a failure to report: there is simply nothing newer.
            404 => {
                tracing::debug!(repo, "no published release");
                return Err(UpdateError::NoAsset);
            }
            code => {
                tracing::warn!(code, repo, "github answered something else");
                return Err(UpdateError::Offline);
            }
        }

        let raw = response.body_mut().read_to_string().map_err(|err| {
            tracing::warn!(error = %err, "could not read the release body");
            UpdateError::Offline
        })?;
        let release: RawRelease = serde_json::from_str(&raw).map_err(|err| {
            tracing::warn!(error = %err, bytes = raw.len(), "release body is not json");
            UpdateError::Offline
        })?;

        Ok(Release {
            tag: release.tag_name,
            html_url: release.html_url,
            assets: release
                .assets
                .into_iter()
                .map(|asset| Asset {
                    name: asset.name,
                    url: asset.browser_download_url,
                    size: asset.size,
                })
                .collect(),
        })
    }

    fn download(&self, asset: &Asset, into: &Path) -> Result<u64, UpdateError> {
        let response = ureq::get(&asset.url)
            .config()
            .timeout_global(Some(PULL_TIMEOUT))
            .http_status_as_error(false)
            .build()
            .header("user-agent", &self.agent)
            .call();

        let mut response = match response {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!(error = %err, "could not reach the download");
                return Err(UpdateError::Download);
            }
        };
        if response.status().as_u16() != 200 {
            tracing::warn!(code = response.status().as_u16(), "download refused");
            return Err(UpdateError::Download);
        }

        let mut file = std::fs::File::create(into).map_err(|err| {
            tracing::warn!(error = %err, "could not open the part file");
            UpdateError::Download
        })?;
        // Bounded rather than read to the end: the cap is what keeps a server
        // answering with something else from filling the disk.
        let mut body = response.body_mut().as_reader().take(MAX_BYTES);
        let written = std::io::copy(&mut body, &mut file).map_err(|err| {
            tracing::warn!(error = %err, "the download stopped part way");
            UpdateError::Download
        })?;

        Ok(written)
    }
}
