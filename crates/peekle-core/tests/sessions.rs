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

    // The capture spans several sessions, so every one of them has to end its
    // turn. Sweeping only the first would leave the rest Running and say
    // nothing about the rule.
    let sessions: Vec<String> = registry
        .cards()
        .iter()
        .map(|card| card.session.session_id.clone())
        .collect();
    for session_id in &sessions {
        registry.end_turn(session_id, 3);
    }

    let cards = registry.cards();
    let entries: Vec<_> = cards
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
                pid: None,
                tty: None,
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
        pid: None,
        tty: None,
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
    let cards = registry.cards();
    let last = cards[0].entries.last().unwrap();
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
                    pid: None,
                    tty: None,
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

#[test]
fn a_status_change_moves_the_session_to_the_front() {
    use peekle_core::types::{SessionRef, SessionStatus};

    let mut registry = SessionRegistry::new();
    for i in 0..3 {
        registry.apply(
            FeedEvent::UserTurn {
                session: SessionRef {
                    session_id: format!("s{i}"),
                    cwd: "/tmp/peekle".into(),
                    project: "peekle".into(),
                    pid: None,
                    tty: None,
                },
                text: format!("turn {i}"),
            },
            i as i64,
        );
    }
    assert_eq!(registry.cards()[0].session.session_id, "s2");

    assert!(registry.set_status("s0", SessionStatus::Idle, 10));
    assert_eq!(registry.cards()[0].session.session_id, "s0");
    assert_eq!(registry.cards()[0].status, SessionStatus::Idle);
}

#[test]
fn a_status_change_on_an_unknown_session_is_reported_not_guessed() {
    use peekle_core::types::SessionStatus;

    let mut registry = SessionRegistry::new();
    assert!(!registry.set_status("nobody", SessionStatus::Idle, 1));
    assert!(registry.cards().is_empty());
}

/// A Stop can be the first event Peekle ever sees for a session, and the
/// prompt still has to have a card to draw into.
#[test]
fn a_session_can_be_opened_by_a_stop_alone() {
    use peekle_core::types::{SessionRef, SessionStatus};

    let mut registry = SessionRegistry::new();
    registry.ensure(
        SessionRef {
            session_id: "s".into(),
            cwd: "/tmp/peekle".into(),
            project: "peekle".into(),
            pid: None,
            tty: None,
        },
        1,
    );
    assert!(registry.set_status("s", SessionStatus::Idle, 2));
    assert_eq!(registry.cards().len(), 1);
    assert!(registry.cards()[0].title.is_empty());
}

/// S5. Two sessions running at once stay separate and keep their own status.
#[test]
fn parallel_sessions_are_kept_apart() {
    use peekle_core::types::{SessionRef, SessionStatus};

    let mut registry = SessionRegistry::new();
    let session = |id: &str, project: &str| SessionRef {
        session_id: id.to_string(),
        cwd: format!("/work/{project}"),
        project: project.to_string(),
        pid: None,
        tty: None,
    };

    registry.apply(
        FeedEvent::UserTurn {
            session: session("a", "peekle"),
            text: "ship the island".into(),
        },
        1,
    );
    registry.apply(
        FeedEvent::UserTurn {
            session: session("b", "other"),
            text: "write the docs".into(),
        },
        2,
    );

    registry.set_status("a", SessionStatus::Idle, 3);
    registry.set_status("b", SessionStatus::Working, 4);

    assert_eq!(registry.cards().len(), 2);

    let cards = registry.cards();
    let a = cards
        .iter()
        .find(|c| c.session.session_id == "a")
        .expect("session a");
    let b = cards
        .iter()
        .find(|c| c.session.session_id == "b")
        .expect("session b");

    assert_eq!(a.status, SessionStatus::Idle);
    assert_eq!(b.status, SessionStatus::Working);
    assert_eq!(a.title, "ship the island");
    assert_eq!(b.title, "write the docs");
    assert_eq!(a.session.project, "peekle");
}

/// SessionEnd has never appeared in a capture without SessionStart preceding
/// it, so a card has to survive being ended without ever being started.
#[test]
fn a_session_can_end_without_ever_having_started() {
    use peekle_core::types::{SessionRef, SessionStatus};

    let mut registry = SessionRegistry::new();
    registry.apply(
        FeedEvent::UserTurn {
            session: SessionRef {
                session_id: "a".into(),
                cwd: "/work/peekle".into(),
                project: "peekle".into(),
                pid: None,
                tty: None,
            },
            text: "go".into(),
        },
        1,
    );

    assert!(registry.set_status("a", SessionStatus::Ended, 2));
    assert_eq!(registry.cards()[0].status, SessionStatus::Ended);
}

/// S6. The closing message lands in the feed, and a Stop that repeats while the
/// user is reading does not say it twice.
#[test]
fn the_closing_message_lands_once_however_often_stop_repeats() {
    use peekle_core::types::{EntryKind, SessionRef};

    let mut registry = SessionRegistry::new();
    let session = SessionRef {
        session_id: "a".into(),
        cwd: "/work/peekle".into(),
        project: "peekle".into(),
        pid: None,
        tty: None,
    };

    registry.assistant_turn(session.clone(), "Tests pass. Want a PR?", 1);
    registry.assistant_turn(session.clone(), "Tests pass. Want a PR?", 2);

    let cards = registry.cards();
    let said: Vec<_> = cards[0]
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Assistant)
        .collect();
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].text, "Tests pass. Want a PR?");

    // A different message is a different thing to say.
    registry.assistant_turn(session, "Opened the PR.", 3);
    assert_eq!(
        registry.cards()[0]
            .entries
            .iter()
            .filter(|e| e.kind == EntryKind::Assistant)
            .count(),
        2
    );
}

#[test]
fn an_empty_closing_message_is_not_an_entry() {
    use peekle_core::types::SessionRef;

    let mut registry = SessionRegistry::new();
    registry.assistant_turn(
        SessionRef {
            session_id: "a".into(),
            cwd: "/work/peekle".into(),
            project: "peekle".into(),
            pid: None,
            tty: None,
        },
        "   \n  ",
        1,
    );
    assert!(registry.cards().is_empty() || registry.cards()[0].entries.is_empty());
}

#[test]
fn a_very_long_closing_message_is_cut_on_a_character_boundary() {
    use peekle_core::types::SessionRef;

    let mut registry = SessionRegistry::new();
    let long = "мысль ".repeat(900);
    registry.assistant_turn(
        SessionRef {
            session_id: "a".into(),
            cwd: "/work/peekle".into(),
            project: "peekle".into(),
            pid: None,
            tty: None,
        },
        &long,
        1,
    );
    assert_eq!(registry.cards()[0].entries[0].text.chars().count(), 2000);
}

/// S3 as of core v29. An answer is a message the user sent, so it lands in the
/// feed; a reply that vanishes on submit reads as one that never went.
#[test]
fn an_answer_lands_in_the_feed_as_a_turn() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/Users/x/peekle".to_string(),
        project: "peekle".to_string(),
        pid: None,
        tty: None,
    };

    registry.user_turn(session.clone(), "  keep going  ", EntryState::Ok, 10);
    let entries = &registry.cards()[0].entries;

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, EntryKind::User);
    assert_eq!(
        entries[0].text, "keep going",
        "trimmed like every other turn"
    );
    assert_eq!(entries[0].state, EntryState::Ok);
}

#[test]
fn an_empty_answer_is_not_a_turn() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/tmp".to_string(),
        project: "tmp".to_string(),
        pid: None,
        tty: None,
    };

    registry.user_turn(session.clone(), "   ", EntryState::Ok, 1);
    assert!(registry.cards().is_empty() || registry.cards()[0].entries.is_empty());
}

/// A session whose agent died would otherwise spin forever: `Working` arrives
/// on a hook and leaves on a hook, and a dead agent sends neither.
#[test]
fn a_session_that_stopped_reporting_goes_back_to_rest() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/tmp".to_string(),
        project: "tmp".to_string(),
        pid: None,
        tty: None,
    };
    registry.ensure(session.clone(), 0);
    registry.set_status("s", peekle_core::types::SessionStatus::Working, 1_000);

    let after = 600_000;
    assert!(
        !registry.rest_stale_work(1_000 + after - 1, after),
        "not yet"
    );
    assert_eq!(
        registry.cards()[0].status,
        peekle_core::types::SessionStatus::Working
    );

    assert!(registry.rest_stale_work(1_000 + after, after));
    assert_eq!(
        registry.cards()[0].status,
        peekle_core::types::SessionStatus::Idle
    );
    assert!(
        !registry.rest_stale_work(9_999_999, after),
        "nothing left to rest"
    );
}

#[test]
fn resting_leaves_every_other_status_alone() {
    use peekle_core::types::SessionStatus;

    for status in [SessionStatus::Idle, SessionStatus::Ended] {
        let mut registry = SessionRegistry::new();
        let session = peekle_core::types::SessionRef {
            session_id: "s".to_string(),
            cwd: "/tmp".to_string(),
            project: "tmp".to_string(),
            pid: None,
            tty: None,
        };
        registry.ensure(session, 0);
        registry.set_status("s", status, 0);

        assert!(!registry.rest_stale_work(i64::MAX / 2, 1), "{status:?}");
        assert_eq!(registry.cards()[0].status, status);
    }
}

/// A reply stays `Running` until `UserPromptSubmit` confirms it, and each
/// event confirms exactly one: the channel is FIFO. tech.md 6.3.
#[test]
fn each_prompt_submit_confirms_one_waiting_reply() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/tmp".to_string(),
        project: "tmp".to_string(),
        pid: None,
        tty: None,
    };

    registry.user_turn(session.clone(), "keep going", EntryState::Running, 1);
    registry.user_turn(session.clone(), "and push", EntryState::Running, 2);
    assert!(registry.cards()[0]
        .entries
        .iter()
        .all(|e| e.state == EntryState::Running));

    assert!(registry.confirm_reply("s", 3));
    let states: Vec<EntryState> = registry.cards()[0]
        .entries
        .iter()
        .map(|e| e.state)
        .collect();
    assert_eq!(
        states,
        vec![EntryState::Ok, EntryState::Running],
        "the oldest one is the one this event belongs to"
    );

    assert!(registry.confirm_reply("s", 4));
    assert!(registry.cards()[0]
        .entries
        .iter()
        .all(|e| e.state == EntryState::Ok));
    assert!(!registry.confirm_reply("s", 5), "nothing left to confirm");
}

/// The reply the island drew when it was typed is the same row the hook
/// confirms. Two rows for one message is what the user reads as sent twice.
#[test]
fn a_prompt_submit_confirms_the_row_instead_of_adding_one() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/tmp".to_string(),
        project: "tmp".to_string(),
        pid: None,
        tty: None,
    };

    registry.user_turn(session.clone(), "1 + 2", EntryState::Running, 1);
    registry.apply(
        peekle_core::FeedEvent::UserTurn {
            session: session.clone(),
            text: "1 + 2".to_string(),
        },
        2,
    );

    let entries = &registry.cards()[0].entries;
    assert_eq!(entries.len(), 1, "one message, one row");
    assert_eq!(entries[0].state, EntryState::Ok);
}

/// A turn the island never sent still needs a row: an observed session, or one
/// the user typed into their own terminal.
#[test]
fn a_turn_the_island_never_sent_still_lands_in_the_feed() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/tmp".to_string(),
        project: "tmp".to_string(),
        pid: None,
        tty: None,
    };

    registry.apply(
        peekle_core::FeedEvent::UserTurn {
            session,
            text: "typed in the terminal".to_string(),
        },
        1,
    );

    let entries = &registry.cards()[0].entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].text, "typed in the terminal");
}

#[test]
fn confirming_leaves_a_session_it_never_heard_of_alone() {
    let mut registry = SessionRegistry::new();
    assert!(!registry.confirm_reply("nobody", 1));
    assert!(registry.cards().is_empty());
}

/// The island opens a session the instant it starts one, and the first hook is
/// a whole agent startup away. Without the card there is nothing to draw, which
/// is what showed up as an empty black island. tech.md 6.5.
#[test]
fn a_session_we_start_has_a_card_before_any_hook_arrives() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "ours".to_string(),
        cwd: "/tmp/project".to_string(),
        project: "project".to_string(),
        pid: None,
        tty: None,
    };

    registry.open_owned(session, 5);

    let cards = registry.cards();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].session.session_id, "ours");
    assert_eq!(
        cards[0].origin,
        peekle_core::types::SessionOrigin::Owned,
        "it is ours, so it has a field"
    );
    assert_eq!(
        cards[0].status,
        peekle_core::types::SessionStatus::Idle,
        "nothing is running yet, the agent is waiting to be spoken to"
    );
    assert!(registry.is_owned("ours"));
}

/// Opening it twice must not stack two cards for one process.
#[test]
fn opening_a_session_we_already_have_moves_it_rather_than_doubling_it() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "ours".to_string(),
        cwd: "/tmp/project".to_string(),
        project: "project".to_string(),
        pid: None,
        tty: None,
    };

    registry.open_owned(session.clone(), 1);
    registry.open_owned(session, 2);
    assert_eq!(registry.cards().len(), 1);
}

/// Seen live: a background task finishing arrives as a UserPromptSubmit and
/// drew a green bubble the user never said. tech.md 6.11.
#[test]
fn what_the_runtime_said_as_the_user_is_not_a_turn() {
    for text in [
        "<task-notification>\n<task-id>b09fngb5b</task-id>\n<status>completed</status>\n</task-notification>",
        "<system-reminder>be careful</system-reminder>",
        "<ide_opened_file>/tmp/x.rs</ide_opened_file>",
        "<local-command-stdout>ok</local-command-stdout>",
        "Caveat: The messages below were generated by the user",
    ] {
        let payload = serde_json::json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "s",
            "cwd": "/tmp",
            "prompt": text,
        });
        assert!(
            peekle_core::FeedEvent::from_payload(&payload).is_none(),
            "the runtime said this, not a person: {text}"
        );
    }
}

/// The filter must not eat a turn that legitimately opens with a tag, which is
/// why the marker list is explicit rather than a shape guess. tech.md 6.11.
#[test]
fn a_turn_that_starts_with_a_tag_is_still_a_turn() {
    for text in [
        "<div> is not rendering, fix it",
        "<T> generics confuse me",
        "почему <task> в коде",
    ] {
        let payload = serde_json::json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "s",
            "cwd": "/tmp",
            "prompt": text,
        });
        assert!(
            peekle_core::FeedEvent::from_payload(&payload).is_some(),
            "a person said this: {text}"
        );
    }
}

/// The worst of the three consequences: a synthetic turn confirming a reply it
/// has nothing to do with, so the island calls delivered what is still flying.
#[test]
fn a_synthetic_turn_does_not_confirm_a_reply_still_in_flight() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/tmp".to_string(),
        project: "tmp".to_string(),
        pid: None,
        tty: None,
    };

    registry.user_turn(session.clone(), "run the tests", EntryState::Running, 1);

    let payload = serde_json::json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": "s",
        "cwd": "/tmp",
        "prompt": "<task-notification><status>completed</status></task-notification>",
    });
    assert!(peekle_core::FeedEvent::from_payload(&payload).is_none());

    let entries = &registry.cards()[0].entries;
    assert_eq!(entries.len(), 1, "no bubble for what nobody said");
    assert_eq!(
        entries[0].state,
        EntryState::Running,
        "the real reply has not been confirmed by someone else's event"
    );
}

/// S14. Titles and hiding live beside the cards, because hooks and the
/// backfill rebuild the cards themselves on every event.
#[test]
fn a_renamed_session_keeps_its_name_through_the_events_that_follow() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/x/peekle".to_string(),
        project: "peekle".to_string(),
        pid: None,
        tty: None,
    };
    registry.ensure(session.clone(), 0);

    assert!(registry.rename("s", "  Panel work  "));
    assert_eq!(registry.cards()[0].title, "Panel work", "trimmed");

    // The event that would otherwise write the first user turn as the title.
    registry.apply(
        FeedEvent::UserTurn {
            session: session.clone(),
            text: "fix the notch".to_string(),
        },
        1,
    );
    assert_eq!(registry.cards()[0].title, "Panel work");

    // And an empty rename hands the name back to the hooks.
    assert!(registry.rename("s", "   "));
    assert_eq!(registry.cards()[0].title, "fix the notch");
}

#[test]
fn renaming_a_session_nobody_knows_is_refused_rather_than_remembered() {
    let mut registry = SessionRegistry::new();
    assert!(!registry.rename("ghost", "Whatever"));
    assert!(registry.overrides().titles.is_empty());
}

#[test]
fn a_hidden_session_stays_hidden_through_events_and_backfill() {
    let mut registry = SessionRegistry::new();
    let session = peekle_core::types::SessionRef {
        session_id: "s".to_string(),
        cwd: "/x/peekle".to_string(),
        project: "peekle".to_string(),
        pid: None,
        tty: None,
    };
    registry.ensure(session.clone(), 0);
    registry.hide("s");
    assert!(registry.cards().is_empty());

    // Its own hooks keep arriving.
    registry.apply(
        FeedEvent::UserTurn {
            session: session.clone(),
            text: "still here".to_string(),
        },
        1,
    );
    assert!(registry.cards().is_empty(), "an event may not raise it");

    // And the backfill offers it again on the next launch.
    registry.seed(vec![peekle_core::types::SessionCard {
        session,
        title: "from the transcript".to_string(),
        status: peekle_core::types::SessionStatus::Idle,
        origin: peekle_core::types::SessionOrigin::Observed,
        entries: Vec::new(),
        agent: None,
        updated_at: 2,
    }]);
    assert!(registry.cards().is_empty(), "the backfill may not raise it");
}

#[test]
fn restoring_applies_what_the_user_said_to_the_cards_already_there() {
    let mut registry = SessionRegistry::new();
    for id in ["a", "b"] {
        registry.ensure(
            peekle_core::types::SessionRef {
                session_id: id.to_string(),
                cwd: "/x/peekle".to_string(),
                project: "peekle".to_string(),
                pid: None,
                tty: None,
            },
            0,
        );
    }

    let mut overrides = peekle_core::sessions::SessionOverrides::default();
    overrides
        .titles
        .insert("a".to_string(), "Renamed".to_string());
    overrides.hidden.insert("b".to_string());
    registry.restore(overrides);

    assert_eq!(registry.cards().len(), 1);
    assert_eq!(registry.cards()[0].title, "Renamed");
}

/// S17. The transcript is the record of the same conversation and carries what
/// no hook does, so it replaces what the events assembled. tech.md 6.11.
mod adopting_a_transcript {
    use super::*;
    use peekle_core::types::{FeedEntry, SessionRef};

    fn session() -> SessionRef {
        SessionRef {
            session_id: "s".into(),
            cwd: "/tmp/peekle".into(),
            project: "peekle".into(),
            pid: None,
            tty: None,
        }
    }

    fn row(id: &str, kind: EntryKind, text: &str) -> FeedEntry {
        FeedEntry {
            id: id.into(),
            kind,
            text: text.into(),
            tool: None,
            detail: None,
            state: EntryState::Ok,
            at: 0,
        }
    }

    fn row_at(id: &str, kind: EntryKind, text: &str, at: i64) -> FeedEntry {
        FeedEntry {
            at,
            ..row(id, kind, text)
        }
    }

    fn registry_with_a_call() -> SessionRegistry {
        let mut registry = SessionRegistry::new();
        registry.apply(
            FeedEvent::ToolStarted {
                session: session(),
                tool_use_id: "toolu_1".into(),
                tool: "Bash".into(),
                preview: "ls".into(),
            },
            0,
        );
        registry
    }

    #[test]
    fn the_file_replaces_what_the_events_assembled() {
        let mut registry = registry_with_a_call();

        assert!(registry.adopt_entries(
            "s",
            vec![
                row("u-1", EntryKind::Assistant, "Here is what I found."),
                row("u-2", EntryKind::Tool, "ls"),
            ],
            None
        ));

        let card = &registry.cards()[0];
        assert_eq!(card.entries.len(), 2);
        assert_eq!(card.entries[0].kind, EntryKind::Assistant);
        assert_eq!(card.entries[0].id, "u-1", "the file names its own rows");
    }

    /// A reply typed in the island sits `Running` until `UserPromptSubmit`
    /// confirms it, and the file has not heard of it yet. Dropping it would
    /// take a sent message off the screen. tech.md 6.11.
    #[test]
    fn a_reply_still_in_flight_survives_the_replacement() {
        let mut registry = registry_with_a_call();
        registry.user_turn(session(), "ship it", EntryState::Running, 1);

        registry.adopt_entries("s", vec![row("u-1", EntryKind::Tool, "ls")], None);

        let card = &registry.cards()[0];
        let last = card.entries.last().unwrap();
        assert_eq!(last.text, "ship it");
        assert_eq!(last.state, EntryState::Running);
    }

    /// Once the file carries it, the local copy has done its job: two of the
    /// same message reads as the user having sent it twice.
    #[test]
    fn a_reply_the_file_has_caught_up_with_is_not_kept_twice() {
        let mut registry = registry_with_a_call();
        registry.user_turn(session(), "ship it", EntryState::Running, 1);

        registry.adopt_entries("s", vec![row("u-1", EntryKind::User, "ship it")], None);

        let card = &registry.cards()[0];
        assert_eq!(card.entries.len(), 1);
        assert_eq!(card.entries[0].id, "u-1");
    }

    /// The bug this closes: the reply goes grey, `UserPromptSubmit` confirms
    /// delivery, the same hook refreshes from a transcript the agent has not
    /// finished writing, and the message the user just sent disappears --
    /// coming back a turn later under a transcript id. Delivery and the file
    /// catching up are two different events, so confirming one must not end
    /// the protection that waits for the other. tech.md 6.11.
    #[test]
    fn a_reply_the_hook_confirmed_survives_a_file_that_has_not_caught_up() {
        let mut registry = registry_with_a_call();
        registry.user_turn(session(), "ship it", EntryState::Running, 1);
        assert!(registry.confirm_reply("s", 2), "the hook confirms delivery");

        registry.adopt_entries("s", vec![row("u-1", EntryKind::Tool, "ls")], None);

        let last = registry.cards()[0].entries.last().unwrap().clone();
        assert_eq!(last.text, "ship it");
        assert_eq!(last.state, EntryState::Ok, "delivered, and still on screen");

        // And it goes when the file finally names it, not twice over.
        registry.adopt_entries(
            "s",
            vec![
                row("u-1", EntryKind::Tool, "ls"),
                row("u-2", EntryKind::User, "ship it"),
            ],
            None,
        );
        let card = &registry.cards()[0];
        assert_eq!(card.entries.len(), 2);
        assert_eq!(card.entries[1].id, "u-2");
    }

    /// The bug this closes: two messages go out one after the other, the file
    /// names the second one and its answer, and the first -- which the file
    /// never named -- slides under that answer. The user watches the message
    /// they just sent jump above the one before it, and the conversation
    /// reads in an order nobody spoke it in. A row the file is missing is not
    /// the newest row in the feed. tech.md 6.11.
    #[test]
    fn a_message_the_file_is_missing_keeps_the_place_it_was_sent_from() {
        let mut registry = registry_with_a_call();
        registry.user_turn(session(), "now then", EntryState::Running, 10);
        registry.user_turn(session(), "testing peekle", EntryState::Running, 20);

        registry.adopt_entries(
            "s",
            vec![
                row_at("u-1", EntryKind::User, "testing peekle", 20),
                row_at("u-2", EntryKind::Assistant, "got it", 30),
            ],
            None,
        );

        let cards = registry.cards();
        let texts: Vec<&str> = cards[0]
            .entries
            .iter()
            .map(|entry| entry.text.as_str())
            .collect();
        assert_eq!(texts, vec!["now then", "testing peekle", "got it"]);
    }

    /// The file's own order is the record of what happened, and a carried row
    /// is woven into it rather than allowed to reshuffle it. tech.md 6.11.
    #[test]
    fn weaving_a_carried_row_in_leaves_the_file_in_its_own_order() {
        let mut registry = registry_with_a_call();
        registry.user_turn(session(), "in between", EntryState::Running, 15);

        registry.adopt_entries(
            "s",
            vec![
                row_at("u-1", EntryKind::User, "before", 10),
                row_at("u-2", EntryKind::Assistant, "after", 20),
                row_at("u-3", EntryKind::Tool, "later still", 30),
            ],
            None,
        );

        let cards = registry.cards();
        let ids: Vec<&str> = cards[0]
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(ids[0], "u-1");
        assert_eq!(ids[2], "u-2");
        assert_eq!(ids[3], "u-3");
        assert_eq!(cards[0].entries[1].text, "in between");
    }

    proptest::proptest! {
        /// However many messages are waiting on the file and whenever they
        /// were sent, the feed reads in the order things were said: the file
        /// keeps its own sequence, and nothing woven into it puts a later row
        /// above an earlier one. tech.md 6.11.
        #[test]
        fn a_woven_feed_still_reads_in_order(
            local in proptest::collection::vec(0i64..100, 0..4),
            file in proptest::collection::vec(0i64..100, 0..8),
        ) {
            let mut registry = registry_with_a_call();
            for (n, at) in local.iter().enumerate() {
                registry.user_turn(session(), &format!("local {n}"), EntryState::Running, *at);
            }

            let mut times = file.clone();
            times.sort_unstable();
            let rows: Vec<FeedEntry> = times
                .iter()
                .enumerate()
                .map(|(n, at)| row_at(&format!("f-{n}"), EntryKind::Assistant, "said", *at))
                .collect();
            registry.adopt_entries("s", rows, None);

            let cards = registry.cards();
            let entries = &cards[0].entries;
            proptest::prop_assert!(entries.windows(2).all(|pair| pair[0].at <= pair[1].at));

            let kept: Vec<&str> = entries
                .iter()
                .map(|entry| entry.id.as_str())
                .filter(|id| id.starts_with("f-"))
                .collect();
            let expected: Vec<String> = (0..times.len()).map(|n| format!("f-{n}")).collect();
            proptest::prop_assert_eq!(kept, expected);
        }
    }

    /// An answer to a permission request is not a prompt: no transcript row
    /// will ever name it, so protecting it would pin it to the bottom of the
    /// feed for the rest of the session. tech.md 6.11.
    #[test]
    fn nothing_but_a_reply_in_flight_survives() {
        let mut registry = registry_with_a_call();
        registry.user_turn(session(), "done already", EntryState::Ok, 1);

        registry.adopt_entries("s", vec![row("u-1", EntryKind::Assistant, "hello")], None);

        let card = &registry.cards()[0];
        assert_eq!(card.entries.len(), 1);
        assert_eq!(card.entries[0].kind, EntryKind::Assistant);
    }

    /// A transcript never opens a card, the same rule the backfill lives by.
    #[test]
    fn a_session_nobody_knows_is_left_alone() {
        let mut registry = SessionRegistry::new();
        assert!(!registry.adopt_entries("nobody", vec![row("u-1", EntryKind::User, "hi")], None));
        assert!(registry.cards().is_empty());
    }

    #[test]
    fn the_adopted_feed_is_capped_like_any_other() {
        let mut registry = registry_with_a_call();
        let rows: Vec<FeedEntry> = (0..(ENTRY_CAP * 2))
            .map(|i| row(&format!("u-{i}"), EntryKind::Tool, "call"))
            .collect();

        registry.adopt_entries("s", rows, None);

        assert_eq!(registry.cards()[0].entries.len(), ENTRY_CAP);
    }
}

/// Captured 2026-09-08 off a headless 2.1.261 session that took a message
/// through its inbox (tech.md 6.5): `UserPromptSubmit` fires in that process
/// with the prompt wrapped as a cross-session message. The turn is the words
/// inside, and it confirms the reply the island put in the feed. tech.md 6.11.
#[test]
fn a_prompt_delivered_through_an_inbox_is_the_words_typed_and_confirms_the_reply() {
    let events = events("user_prompt_submit_peer.jsonl");
    assert_eq!(events.len(), 1, "{events:?}");
    let FeedEvent::UserTurn { session, text } = &events[0] else {
        panic!("UserPromptSubmit produced {:?}", events[0]);
    };
    assert_eq!(
        text,
        "Probe from peekle-68: reply with exactly the word PONG and nothing else."
    );
    assert_eq!(session.session_id, "11111111-2222-4333-8444-555555555555");

    let mut registry = SessionRegistry::new();
    registry.user_turn(session.clone(), text, EntryState::Running, 1);
    assert!(registry.confirm_reply(&session.session_id, 2));
    let card = &registry.cards()[0];
    assert_eq!(card.entries.len(), 1);
    assert_eq!(card.entries[0].state, EntryState::Ok);
    assert_eq!(card.entries[0].text, *text);
}
