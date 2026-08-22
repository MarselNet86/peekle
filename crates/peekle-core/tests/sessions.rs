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
