#![allow(clippy::unwrap_used)]
//! S13 against a captured transcript. The fixture is a real file with the
//! conversation redacted: every key, type, order and length survives, the text
//! does not. tech.md 6.11.

use peekle_core::sessions::{SessionRegistry, ENTRY_CAP, SESSION_CAP};
use peekle_core::transcripts::{card_from_lines, scan};
use peekle_core::types::{EntryKind, EntryState, SessionStatus};

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

/// The IDE, the CLI and the runtime all write into the transcript wearing the
/// user's role. None of it is a turn. tech.md 6.11.
#[test]
fn what_the_ide_and_the_cli_injected_is_not_a_turn() {
    use peekle_core::transcripts::is_synthetic;

    for injected in [
        "<ide_opened_file>The user opened the file /tmp/x.rs",
        "  <ide_selection>lines 3 to 9",
        "<system-reminder>remember the rules</system-reminder>",
        "<command-name>/compact</command-name>",
        "<local-command-stdout>Compacted</local-command-stdout>",
        "<user-prompt-submit-hook>ran</user-prompt-submit-hook>",
        "Caveat: The messages below were generated by the user while running local commands",
    ] {
        assert!(is_synthetic(injected), "{injected}");
    }
}

#[test]
fn a_turn_that_merely_starts_with_a_tag_is_still_a_turn() {
    use peekle_core::transcripts::is_synthetic;

    for spoken in [
        "<div> renders twice, fix it",
        "<Shape> should not resize the window",
        "почему <ide_opened_file> попадает в ленту?",
        "",
    ] {
        assert!(!is_synthetic(spoken), "{spoken}");
    }
}

#[test]
fn a_session_of_nothing_but_injections_is_not_a_session() {
    let lines = [
        r#"{"type":"user","sessionId":"s","message":{"content":[{"type":"text","text":"<ide_opened_file>x"}]}}"#,
        r#"{"type":"user","sessionId":"s","message":{"content":[{"type":"text","text":"<system-reminder>y"}]}}"#,
    ];
    assert!(card_from_lines(lines, "s", 0).is_none());
}

#[test]
fn an_injection_never_becomes_the_title() {
    let lines = [
        r#"{"type":"user","sessionId":"s","message":{"content":[{"type":"text","text":"<ide_opened_file>/tmp/x.rs"}]}}"#,
        r#"{"type":"user","sessionId":"s","message":{"content":[{"type":"text","text":"fix the scroll"}]}}"#,
    ];
    let card = card_from_lines(lines, "s", 0).unwrap();

    assert_eq!(card.title, "fix the scroll");
    assert_eq!(card.entries.len(), 1);
}

/// The same objects the terminal shows, in the same shapes. tech.md 6.11.
#[test]
fn every_kind_of_object_comes_back() {
    let card = card();
    let count = |kind: EntryKind| card.entries.iter().filter(|e| e.kind == kind).count();

    // The fixture holds six records with the user role, but five of them are
    // tool results: Claude Code writes those back as user turns, and a result
    // belongs to its call rather than to the conversation.
    assert_eq!(count(EntryKind::User), 1, "a tool result is not a turn");
    assert_eq!(count(EntryKind::Assistant), 1);
    assert_eq!(count(EntryKind::Thought), 5);
    assert_eq!(count(EntryKind::Tool), 5);
}

#[test]
fn a_thought_is_a_marker_with_its_reasoning_behind_it() {
    let card = card();
    let thought = card
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::Thought)
        .expect("the fixture thinks");

    assert!(thought.text.starts_with("Thought for "), "{}", thought.text);
    assert!(thought.text.ends_with('s'));
    // The reasoning itself is often not in the file at all: Claude Code stores
    // a signature and an empty string. The marker is what survives, and that is
    // also all the terminal shows without an expansion. tech.md 6.11.
    assert!(card
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Thought)
        .all(|e| e.text.starts_with("Thought for ")));
}

#[test]
fn reasoning_that_is_in_the_file_lands_behind_the_marker() {
    let lines = [
        r#"{"type":"assistant","sessionId":"s","timestamp":"2026-08-17T14:55:20.000Z","message":{"content":[{"type":"text","text":"go"}]}}"#,
        r#"{"type":"assistant","sessionId":"s","timestamp":"2026-08-17T14:55:32.000Z","message":{"content":[{"type":"thinking","thinking":"weighing two options"}]}}"#,
    ];
    let card = card_from_lines(lines, "s", 0).unwrap();
    let thought = card
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::Thought)
        .unwrap();

    assert_eq!(thought.text, "Thought for 12s");
    assert_eq!(thought.detail.as_deref(), Some("weighing two options"));
}

#[test]
fn a_tool_call_carries_its_name_its_input_and_its_result() {
    let card = card();
    let calls: Vec<_> = card
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Tool)
        .collect();

    assert!(calls.iter().all(|c| c.tool.is_some()));
    assert!(
        calls.iter().all(|c| c.detail.is_some()),
        "the input is behind the row"
    );
    // Every call in this fixture reported back, so none is left running.
    assert!(calls.iter().all(|c| c.state != EntryState::Running));
}

/// A call whose result never arrived is the one case that stays open, and
/// saying it succeeded would be a claim nobody made. tech.md 6.3.
#[test]
fn a_call_without_a_result_stays_running() {
    let lines = [
        r#"{"type":"user","sessionId":"s","message":{"content":[{"type":"text","text":"go"}]}}"#,
        r#"{"type":"assistant","sessionId":"s","message":{"content":[{"type":"tool_use","id":"a","name":"Bash","input":{"command":"cargo test"}}]}}"#,
    ];
    let card = card_from_lines(lines, "s", 0).unwrap();
    let call = card
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::Tool)
        .unwrap();

    assert_eq!(call.state, EntryState::Running);
    assert_eq!(call.text, "cargo test", "the collapsed row is the command");
    assert_eq!(call.tool.as_deref(), Some("Bash"));
}

#[test]
fn a_failed_result_marks_its_call_failed() {
    let lines = [
        r#"{"type":"assistant","sessionId":"s","message":{"content":[{"type":"tool_use","id":"a","name":"Bash","input":{"command":"exit 42"}}]}}"#,
        r#"{"type":"user","sessionId":"s","message":{"content":[{"type":"tool_result","tool_use_id":"a","is_error":true,"content":"exit code 42"}]}}"#,
    ];
    let card = card_from_lines(lines, "s", 0).unwrap();
    let call = card
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::Tool)
        .unwrap();

    assert_eq!(call.state, EntryState::Failed);
    assert!(call
        .detail
        .as_deref()
        .unwrap_or_default()
        .contains("exit code 42"));
}
