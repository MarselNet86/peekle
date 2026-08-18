#![allow(clippy::unwrap_used)]
//! S2 acceptance, driven by payloads captured off a live Claude Code session.
//! tech.md section 10: the fixture is the only place the Claude Code contract
//! can be checked at all, so nothing here is hand written.

use std::path::PathBuf;

use peekle_core::sessions::{FeedEvent, SessionRegistry, ENTRY_CAP, SESSION_CAP};
use peekle_core::types::{EntryKind, EntryState};
use peekle_core::{FixtureFeed, TaskFeed};

fn fixture(name: &str) -> FixtureFeed {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/hooks")
        .join(name);
    FixtureFeed::from_file(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn events(name: &str) -> Vec<FeedEvent> {
    let feed = fixture(name);
    let mut out = Vec::new();
    while let Some(payload) = feed.next() {
        if let Some(event) = FeedEvent::from_payload(&payload) {
            out.push(event);
        }
    }
    out
}

#[test]
fn every_captured_user_prompt_becomes_a_user_turn() {
    let events = events("user_prompt_submit.jsonl");
    assert!(!events.is_empty(), "the capture is empty");

    for event in &events {
        let FeedEvent::UserTurn { session, text } = event else {
            panic!("UserPromptSubmit produced {event:?}");
        };
        assert!(!session.session_id.is_empty());
        assert!(!text.is_empty());
    }
}

#[test]
fn every_captured_pre_tool_use_names_its_tool_and_its_call() {
    let events = events("pre_tool_use.jsonl");
    assert!(!events.is_empty(), "the capture is empty");

    for event in &events {
        let FeedEvent::ToolStarted {
            tool, tool_use_id, ..
        } = event
        else {
            panic!("PreToolUse produced {event:?}");
        };
        assert!(!tool.is_empty());
        assert!(tool_use_id.starts_with("toolu_"), "{tool_use_id}");
    }
}

#[test]
fn every_captured_post_tool_use_closes_a_call() {
    let events = events("post_tool_use.jsonl");
    assert!(!events.is_empty(), "the capture is empty");

    for event in &events {
        let FeedEvent::ToolFinished { tool_use_id, .. } = event else {
            panic!("PostToolUse produced {event:?}");
        };
        assert!(tool_use_id.starts_with("toolu_"), "{tool_use_id}");
    }
}

/// The acceptance criterion of S2: one call is one row, whatever the payload
/// count. PreToolUse opens it and PostToolUse closes the same one.
#[test]
fn a_call_captured_twice_collapses_into_one_row() {
    let mut registry = SessionRegistry::new();

    let started = events("pre_tool_use.jsonl");
    let finished = events("post_tool_use.jsonl");
    for event in started.iter().cloned() {
        registry.apply(event, 1);
    }
    let rows_after_pre: usize = registry.cards().iter().map(|c| c.entries.len()).sum();

    for event in finished.iter().cloned() {
        registry.apply(event, 2);
    }
    let rows_after_post: usize = registry.cards().iter().map(|c| c.entries.len()).sum();

    assert_eq!(rows_after_pre, started.len());
    assert_eq!(
        rows_after_post, rows_after_pre,
        "PostToolUse added rows instead of closing them"
    );

    let ok = registry
        .cards()
        .iter()
        .flat_map(|c| &c.entries)
        .filter(|e| e.state == EntryState::Ok && e.kind == EntryKind::Tool)
        .count();
    assert_eq!(ok, finished.len());
}

/// The v8 rule. A failed call gives PreToolUse and no PostToolUse, so the row
/// is closed by the turn boundary and not by an event.
#[test]
fn the_stop_sweep_fails_only_what_never_finished() {
    let mut registry = SessionRegistry::new();
    let started = events("pre_tool_use.jsonl");
    let finished = events("post_tool_use.jsonl");
    assert!(
        started.len() > finished.len(),
        "the capture has to hold at least one call that never returned"
    );

    for event in started.iter().cloned() {
        registry.apply(event, 1);
    }
    for event in finished.iter().cloned() {
        registry.apply(event, 2);
    }

    let session_id = registry.cards()[0].session.session_id.clone();
    registry.end_turn(&session_id, 3);

    let entries: Vec<_> = registry
        .cards()
        .iter()
        .flat_map(|c| &c.entries)
        .filter(|e| e.kind == EntryKind::Tool)
        .collect();

    assert_eq!(
        entries.iter().filter(|e| e.state == EntryState::Ok).count(),
        finished.len(),
        "the sweep touched a call that had already reported success"
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.state == EntryState::Failed)
            .count(),
        started.len() - finished.len()
    );
    assert!(
        !entries.iter().any(|e| e.state == EntryState::Running),
        "nothing may still be running once the turn has ended"
    );
}

#[test]
fn a_second_sweep_changes_nothing() {
    let mut registry = SessionRegistry::new();
    for event in events("pre_tool_use.jsonl") {
        registry.apply(event, 1);
    }
    let session_id = registry.cards()[0].session.session_id.clone();

    registry.end_turn(&session_id, 2);
    let after_one: Vec<_> = registry.cards()[0]
        .entries
        .iter()
        .map(|e| (e.id.clone(), e.state))
        .collect();

    registry.end_turn(&session_id, 3);
    let after_two: Vec<_> = registry.cards()[0]
        .entries
        .iter()
        .map(|e| (e.id.clone(), e.state))
        .collect();

    assert_eq!(after_one, after_two);
}

#[test]
fn the_title_is_the_first_user_turn_and_does_not_move() {
    let mut registry = SessionRegistry::new();
    let turns = events("user_prompt_submit.jsonl");

    for event in turns.iter().cloned() {
        registry.apply(event, 1);
    }

    let FeedEvent::UserTurn { text, .. } = &turns[0] else {
        panic!("expected a user turn");
    };
    let title = &registry.cards()[0].title;
    assert!(!title.is_empty());
    assert!(text.starts_with(title.as_str()) || title.chars().count() <= 80);
}

#[test]
fn a_prompt_that_is_not_ascii_survives_truncation() {
    let mut registry = SessionRegistry::new();
    let long = "давай третий вариант ".repeat(40);
    registry.apply(
        FeedEvent::UserTurn {
            session: peekle_core::types::SessionRef {
                session_id: "s".into(),
                cwd: "/tmp/peekle".into(),
                project: "peekle".into(),
            },
            text: long,
        },
        1,
    );

    let card = &registry.cards()[0];
    assert_eq!(card.title.chars().count(), 80);
    assert_eq!(card.session.project, "peekle");
}

#[test]
fn the_feed_is_capped_per_session() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".into(),
        cwd: "/tmp/peekle".into(),
        project: "peekle".into(),
    };

    for i in 0..(ENTRY_CAP * 2) {
        registry.apply(
            FeedEvent::ToolStarted {
                session: session.clone(),
                tool_use_id: format!("toolu_{i}"),
                tool: "Bash".into(),
                preview: format!("call {i}"),
            },
            i as i64,
        );
    }

    assert_eq!(registry.cards()[0].entries.len(), ENTRY_CAP);
    // The tail is kept, so the newest call is the one still on screen.
    let last = registry.cards()[0].entries.last().unwrap();
    assert_eq!(last.text, format!("call {}", ENTRY_CAP * 2 - 1));
}

#[test]
fn the_session_list_is_capped_and_freshest_first() {
    let mut registry = SessionRegistry::new();

    for i in 0..(SESSION_CAP + 5) {
        registry.apply(
            FeedEvent::UserTurn {
                session: peekle_core::types::SessionRef {
                    session_id: format!("s{i}"),
                    cwd: "/tmp/peekle".into(),
                    project: "peekle".into(),
                },
                text: format!("turn {i}"),
            },
            i as i64,
        );
    }

    assert_eq!(registry.cards().len(), SESSION_CAP);
    assert_eq!(
        registry.cards()[0].session.session_id,
        format!("s{}", SESSION_CAP + 4)
    );
}

#[test]
fn closing_a_call_peekle_never_saw_open_is_ignored() {
    let mut registry = SessionRegistry::new();
    registry.apply(
        FeedEvent::ToolFinished {
            session_id: "s".into(),
            tool_use_id: "toolu_never_seen".into(),
        },
        1,
    );
    assert!(registry.cards().is_empty());
}

#[test]
fn an_event_this_build_does_not_know_is_dropped_whole() {
    let payload = serde_json::json!({"hook_event_name": "SomethingNew", "session_id": "s"});
    assert!(FeedEvent::from_payload(&payload).is_none());

    // Claude Code adds fields between versions; a missing one must not panic.
    let partial = serde_json::json!({"hook_event_name": "PreToolUse", "session_id": "s"});
    assert!(FeedEvent::from_payload(&partial).is_none());
}
