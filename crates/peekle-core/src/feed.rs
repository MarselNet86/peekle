//! Task feed. tech.md section 7.
//!
//! The real feed is the hook endpoints. The fake replays captured payloads
//! from `fixtures/hooks/*.jsonl` in order, so the HUD can be driven without a
//! live Claude Code session and without hand-written payloads.

use std::path::Path;
use std::sync::Mutex;

use serde_json::Value;

pub trait TaskFeed: Send + Sync + 'static {
    /// The next captured payload, or None once the recording runs out.
    fn next(&self) -> Option<Value>;
}

/// Replays one captured `.jsonl` file. Each line is one payload exactly as it
/// arrived on the wire.
#[derive(Debug, Default)]
pub struct FixtureFeed {
    payloads: Vec<Value>,
    cursor: Mutex<usize>,
}

impl FixtureFeed {
    pub fn from_jsonl(text: &str) -> Self {
        let payloads = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        Self {
            payloads,
            cursor: Mutex::new(0),
        }
    }

    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        Ok(Self::from_jsonl(&std::fs::read_to_string(path)?))
    }

    pub fn len(&self) -> usize {
        self.payloads.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }

    pub fn rewind(&self) {
        *self.lock() = 0;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, usize> {
        self.cursor.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl TaskFeed for FixtureFeed {
    fn next(&self) -> Option<Value> {
        let mut cursor = self.lock();
        let payload = self.payloads.get(*cursor).cloned();
        if payload.is_some() {
            *cursor += 1;
        }
        payload
    }
}

/// What the agent said last, capped. tech.md 6.3.
pub const LAST_MESSAGE_LIMIT: usize = 2000;

/// Truncates on a character boundary and marks the cut, so nothing is ever
/// shortened silently. Character counts, not bytes: cutting mid-codepoint
/// would corrupt the text rather than shorten it.
pub fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replays_captured_lines_in_order() {
        let feed = FixtureFeed::from_jsonl("{\"a\":1}\n{\"a\":2}\n");

        assert_eq!(feed.len(), 2);
        assert_eq!(feed.next(), Some(json!({"a": 1})));
        assert_eq!(feed.next(), Some(json!({"a": 2})));
        assert_eq!(feed.next(), None);
    }

    #[test]
    fn rewinds_for_a_second_pass() {
        let feed = FixtureFeed::from_jsonl("{\"a\":1}\n");
        assert!(feed.next().is_some());
        feed.rewind();
        assert!(feed.next().is_some());
    }

    /// A truncated capture must not take the feed down: the recording is a
    /// file on disk and can be cut mid line.
    #[test]
    fn skips_lines_that_are_not_json() {
        let feed = FixtureFeed::from_jsonl("{\"a\":1}\nnot json\n{\"a\":2}\n");
        assert_eq!(feed.len(), 2);
    }

    #[test]
    fn an_empty_recording_is_empty_not_an_error() {
        let feed = FixtureFeed::from_jsonl("\n\n");
        assert!(feed.is_empty());
        assert_eq!(feed.next(), None);
    }
}
