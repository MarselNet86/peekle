#![allow(clippy::unwrap_used)]
//! S13 against a captured transcript. The fixture is a real file with the
//! conversation redacted: every key, type, order and length survives, the text
//! does not. tech.md 6.11.

use peekle_core::sessions::{SessionRegistry, ENTRY_CAP, SESSION_CAP};
use peekle_core::transcripts::{card_from_lines, scan};
use peekle_core::types::{EntryKind, SessionStatus};

const FIXTURE: &str = include_str!("../../../fixtures/transcripts/session.jsonl");

fn card() -> peekle_core::types::SessionCard {
    card_from_lines(FIXTURE.lines(), "fallback", 0).expect("the fixture carries a dialogue")
}

#[test]
fn a_captured_transcript_becomes_a_card() {
    let card = card();

    assert_eq!(
        card.session.session_id,
        "c64242ae-b10a-4ca6-966c-7f2a56b395bd"
    );
    assert_eq!(card.session.project, "peekle");
    assert_eq!(card.session.cwd, "/Users/marsel.shamsutdinov/peekle");
    // Nothing in a file says whether the session ended or was killed.
    assert_eq!(card.status, SessionStatus::Idle);
    assert!(card.updated_at > 0);
}

/// Claude Code writes its own title for its own list, so reusing it beats
/// cutting the first line in half.
#[test]
fn the_title_comes_from_the_one_claude_code_wrote() {
    assert_eq!(card().title, "xxxxxxxx xxxxx xxxxxxxxx");
}

#[test]
fn a_transcript_without_a_title_falls_back_to_the_first_turn() {
    let lines: Vec<String> = FIXTURE
        .lines()
        .filter(|line| !line.contains("\"ai-title\""))
        .map(str::to_string)
        .collect();

    let card = card_from_lines(lines.iter(), "fallback", 0).unwrap();
    assert!(card.title.starts_with('x'), "{}", card.title);
    assert!(!card.title.is_empty());
}

/// Thinking is the agent reasoning with itself and the transcript is the only
/// place it is written down. It never reaches this surface. tech.md 6.11.
#[test]
fn only_the_spoken_turns_come_back() {
    let card = card();

    assert!(card.entries.iter().all(|e| e.tool.is_none()));
    assert!(card
        .entries
        .iter()
        .all(|e| matches!(e.kind, EntryKind::User | EntryKind::Assistant)));

    let users = card
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::User)
        .count();
    let assistants = card
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Assistant)
        .count();

    // The fixture holds six records with the user role, but five of them are
    // tool results: Claude Code writes those back as user turns. Only one is
    // something a person said. The assistant side is one text block among five
    // thinking blocks and five tool calls.
    assert_eq!(users, 1, "a tool result is not a turn");
    assert_eq!(assistants, 1);
}

#[test]
fn the_entries_are_in_the_order_they_happened() {
    let card = card();
    let times: Vec<i64> = card.entries.iter().map(|e| e.at).collect();
    let mut sorted = times.clone();
    sorted.sort_unstable();
    assert_eq!(times, sorted);
}

/// A half written last line is normal: Claude Code appends while we read.
#[test]
fn a_broken_line_costs_that_line_and_nothing_else() {
    let mut lines: Vec<String> = FIXTURE.lines().map(str::to_string).collect();
    lines.insert(3, "{not json at all".to_string());
    lines.push("{\"type\":\"user\",\"message\":".to_string());

    let whole = card();
    let damaged = card_from_lines(lines.iter(), "fallback", 0).unwrap();
    assert_eq!(damaged.entries.len(), whole.entries.len());
}

#[test]
fn a_file_with_no_dialogue_yields_no_card() {
    assert!(card_from_lines(Vec::<String>::new(), "id", 0).is_none());
    assert!(card_from_lines(["", "   ", "{}"], "id", 0).is_none());
    // Records that carry no turn at all are not a session worth a row.
    assert!(card_from_lines(["{\"type\":\"queue-operation\"}"], "id", 0).is_none());
}

#[test]
fn a_transcript_that_never_named_its_session_falls_back_to_the_file_name() {
    let line = r#"{"type":"user","message":{"content":[{"type":"text","text":"hi"}]}}"#;
    let card = card_from_lines([line], "from-the-file-name", 7).unwrap();

    assert_eq!(card.session.session_id, "from-the-file-name");
    assert_eq!(card.updated_at, 7);
}

#[test]
fn the_entry_tail_is_capped_like_the_live_feed() {
    let line =
        r#"{"type":"user","sessionId":"s","message":{"content":[{"type":"text","text":"hi"}]}}"#;
    let lines: Vec<&str> = std::iter::repeat_n(line, ENTRY_CAP + 40).collect();

    let card = card_from_lines(lines, "s", 0).unwrap();
    assert_eq!(card.entries.len(), ENTRY_CAP);
}

#[test]
fn an_empty_or_missing_directory_is_not_an_error() {
    assert!(scan(std::path::Path::new("/no/such/place"), 0).is_empty());

    let dir = std::env::temp_dir().join(format!("peekle-scan-{}", ulid::Ulid::generate()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(scan(&dir, 0).is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A live hook always wins: the backfill fills gaps and never overwrites what
/// arrived over the wire. tech.md 6.11.
#[test]
fn seeding_leaves_a_session_the_hooks_already_know_alone() {
    let mut registry = SessionRegistry::new();
    let live = peekle_core::types::SessionRef {
        session_id: "c64242ae-b10a-4ca6-966c-7f2a56b395bd".to_string(),
        cwd: "/Users/x/peekle".to_string(),
        project: "peekle".to_string(),
    };
    registry.ensure(live.clone(), 1000);
    registry.set_status(&live.session_id, SessionStatus::Working, 1000);

    registry.seed(vec![card()]);

    assert_eq!(
        registry.cards().len(),
        1,
        "the same session twice is one card"
    );
    assert_eq!(registry.cards()[0].status, SessionStatus::Working);
}

#[test]
fn seeding_adds_what_the_hooks_never_saw_and_respects_the_cap() {
    let mut registry = SessionRegistry::new();

    let cards: Vec<peekle_core::types::SessionCard> = (0..SESSION_CAP + 10)
        .map(|index| {
            let mut card = card();
            card.session.session_id = format!("s{index}");
            card.updated_at = index as i64;
            card
        })
        .collect();

    registry.seed(cards);
    assert_eq!(registry.cards().len(), SESSION_CAP);
    // Freshest first, like every other path into the registry. tech.md 6.3.
    assert_eq!(registry.cards()[0].session.session_id, "s29");
}
