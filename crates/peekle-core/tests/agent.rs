#![allow(clippy::unwrap_used)]
//! S18 against captured transcripts. Both fixtures are real files with the
//! conversation redacted: every key, type, order and number survives, the text
//! does not. tech.md 6.15.

use peekle_core::agent::{self, CommandError, DEFAULT_WINDOW};
use peekle_core::transcripts::{agent_from_lines, card_from_lines};
use peekle_core::types::Effort;
use proptest::prelude::*;

/// A short session, captured whole.
const DIALOGUE: &str = include_str!("../../../fixtures/transcripts/agent.jsonl");
/// The `system` records of a session far too large to be a fixture, captured
/// for the one record type that carries a compact.
const COMPACTS: &str = include_str!("../../../fixtures/transcripts/compact.jsonl");

#[test]
fn a_captured_transcript_names_what_it_answers_with() {
    let setup = agent_from_lines(DIALOGUE.lines()).expect("the fixture had a turn");

    assert_eq!(setup.model.as_deref(), Some("claude-sonnet-5"));
    assert_eq!(setup.label.as_deref(), Some("Sonnet 5"));
    assert_eq!(setup.effort, Some(Effort::High));
    assert_eq!(setup.context_tokens, 41_972);
    assert_eq!(setup.context_window, 1_000_000);
    assert!((setup.context_pct - 4.1972).abs() < 0.001, "{setup:?}");
}

/// The card and the row come out of the same pass, so a session opened in the
/// island shows what its own file says.
#[test]
fn the_card_carries_the_same_reading() {
    let card = card_from_lines(DIALOGUE.lines(), "fallback", 0).expect("a dialogue");
    assert_eq!(card.agent, agent_from_lines(DIALOGUE.lines()));
}

/// The last request is the state the session is in. An earlier one is history,
/// and history is what the feed is for.
#[test]
fn the_last_record_is_the_one_that_counts() {
    let mut lines: Vec<String> = DIALOGUE.lines().map(str::to_string).collect();
    // The same file with one more answer on the end, taken from itself: same
    // shape, a reading that moved.
    let last = lines
        .iter()
        .rev()
        .find(|line| line.contains("\"assistant\""))
        .expect("the fixture answers at least once")
        .replace("\"input_tokens\": 2", "\"input_tokens\": 60002")
        .replace("claude-sonnet-5", "claude-haiku-4-5");
    lines.push(last);

    let setup = agent_from_lines(lines.iter()).expect("still a reading");
    assert_eq!(setup.label.as_deref(), Some("Haiku 4.5"));
    assert_eq!(setup.context_tokens, 101_972);
    // Haiku takes no effort, so the row says nothing about it rather than
    // repeating what the file still carries.
    assert_eq!(setup.effort, None);
    assert_eq!(setup.context_window, 200_000);
}

/// A file nobody has answered in yet has no reading. Zeroes would claim an
/// empty context, which is a different statement from not knowing.
#[test]
fn a_transcript_without_an_answer_reports_nothing() {
    assert_eq!(agent_from_lines(COMPACTS.lines()), None);
    assert_eq!(agent_from_lines(Vec::<String>::new()), None);
    assert_eq!(agent_from_lines(["", "   ", "{}", "not json"]), None);
}

/// A compact somebody asked for says nothing about the size of the window.
/// Only an automatic one, which fires near the limit, is evidence.
#[test]
fn a_manual_compact_leaves_the_window_alone() {
    let manual: Vec<String> = COMPACTS
        .lines()
        .filter(|line| line.contains("compactMetadata"))
        .map(str::to_string)
        .collect();
    assert!(!manual.is_empty(), "the fixture carries compact records");
    assert!(
        manual
            .iter()
            .all(|line| line.contains("\"trigger\": \"manual\"")),
        "the captured compacts are the manual ones"
    );

    let answer = DIALOGUE
        .lines()
        .find(|line| line.contains("\"assistant\""))
        .expect("an answer to attach them to")
        .to_string();
    let mut lines = manual;
    lines.push(answer);

    let setup = agent_from_lines(lines.iter()).expect("a reading");
    assert_eq!(
        setup.context_window, 1_000_000,
        "a manual compact is not the shape of a window"
    );
}

/// The commands are the whole write. Anything that could carry a second line
/// is refused rather than escaped: a pty is a stream, and a newline inside an
/// argument is a message sent to the agent. tech.md 6.15.
#[test]
fn the_commands_carry_one_argument_and_no_more() {
    for choice in agent::choices() {
        let line = agent::model_command(&choice.alias).expect("a catalog alias is sane");
        assert_eq!(line, format!("/model {}", choice.alias));
        assert_eq!(line.lines().count(), 1);
    }
    for level in Effort::ALL {
        let line = agent::effort_command(level);
        assert_eq!(line, format!("/effort {}", level.flag()));
        assert_eq!(line.lines().count(), 1);
    }
    assert_eq!(agent::COMPACT_COMMAND.lines().count(), 1);
}

proptest! {
    /// The ring is drawn from these two numbers on every event, so they have
    /// to be sane whatever a transcript, a catalog miss or a compact says.
    #[test]
    fn a_reading_always_fits_its_ring(
        model in "(claude-)?[a-z0-9\\-]{0,30}",
        effort in "[a-z]{0,8}",
        tokens in 0u32..u32::MAX,
        compact in proptest::option::of(0u32..u32::MAX),
    ) {
        let setup = agent::setup(Some(&model), Some(&effort), tokens, compact);
        prop_assert!(setup.context_window > 0);
        prop_assert!((0.0..=100.0).contains(&setup.context_pct));
        prop_assert!(!setup.context_pct.is_nan());
    }

    /// The correction only ever lowers. A compact cannot hand a session a
    /// window bigger than the model has.
    #[test]
    fn the_window_never_grows_past_the_catalog(compact in proptest::option::of(0u32..u32::MAX)) {
        for model in ["claude-opus-5", "claude-haiku-4-5", "claude-tomorrow-9"] {
            let catalogue = agent::window_for(Some(model), None);
            let corrected = agent::window_for(Some(model), compact);
            prop_assert!(corrected <= catalogue, "{model}: {corrected} > {catalogue}");
            prop_assert!(corrected > 0);
        }
    }

    /// A model name is either sent whole or refused. Nothing is rewritten on
    /// the way out, because a rewritten name is a model the user did not pick.
    #[test]
    fn a_model_name_is_sent_whole_or_not_at_all(name in ".*") {
        match agent::model_command(&name) {
            Ok(line) => {
                prop_assert_eq!(line, format!("/model {}", name.trim()));
            }
            Err(CommandError::BadModel) => {}
        }
    }
}

/// An unknown model still measures against something: the default Claude Code
/// itself falls back to. tech.md 6.15.
#[test]
fn tomorrows_model_still_draws_a_ring() {
    let setup = agent::setup(Some("claude-tomorrow-9"), None, 100_000, None);
    assert_eq!(setup.label, None);
    assert_eq!(setup.context_window, DEFAULT_WINDOW);
    assert_eq!(setup.context_pct, 50.0);
}
