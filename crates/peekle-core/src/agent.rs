//! What a session answers with: its model, its effort, and how full its
//! context window is. tech.md 6.15.
//!
//! None of the three arrives in a hook. They are read out of the transcript,
//! and they are changed by writing a slash command into the pty of a session
//! Peekle owns -- the same channel a reply takes, because a slash command is
//! text like any other.
//!
//! Everything here is pure. The bytes reach the agent through `pty::PtyHost`.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::types::{AgentSetup, Effort, ModelChoice};

/// Claude Code's own model catalog, captured from the binary by
/// `scripts/capture-models.sh`. The window a ring measures against is written
/// down in exactly one place, and this is a copy of it rather than a guess.
/// tech.md 6.15.
const CATALOG: &str = include_str!("../../../fixtures/models/catalog.json");

/// What Claude Code measures a model it does not know against, and so do we.
pub const DEFAULT_WINDOW: u32 = 200_000;

/// The families the picker offers, in the order Claude Code offers them. The
/// alias is the family name: `/model opus` is what the CLI itself takes.
pub const FAMILIES: &[&str] = &["fable", "opus", "sonnet", "haiku"];

/// One model of the catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: String,
    pub family: String,
    /// What the picker calls it: "Opus 5".
    pub label: String,
    pub context_window: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl Model {
    /// Whether `/effort` means anything to this model at all. Haiku 4.5 takes
    /// no effort, and offering it would send a command the agent drops.
    pub fn takes_effort(&self) -> bool {
        self.has("effort")
    }

    /// The levels this model accepts, weakest first.
    pub fn levels(&self) -> Vec<Effort> {
        if !self.takes_effort() {
            return Vec::new();
        }
        let mut levels = vec![Effort::Low, Effort::Medium, Effort::High];
        if self.has("xhigh_effort") {
            levels.push(Effort::XHigh);
        }
        if self.has("max_effort") {
            levels.push(Effort::Max);
        }
        levels
    }

    fn has(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|each| each == capability)
    }
}

#[derive(Debug, Deserialize)]
struct Catalog {
    models: Vec<Model>,
}

/// The catalog, parsed once. A file that will not parse leaves an empty
/// catalog: every model then reads as unknown and measures against the
/// default, which is what the CLI does with a model it has never heard of.
pub fn catalog() -> &'static [Model] {
    static PARSED: OnceLock<Vec<Model>> = OnceLock::new();
    PARSED.get_or_init(|| match serde_json::from_str::<Catalog>(CATALOG) {
        Ok(catalog) => catalog.models,
        Err(err) => {
            tracing::warn!(error = %err, "the model catalog would not parse");
            Vec::new()
        }
    })
}

/// The catalog entry for what a transcript calls the model.
///
/// Exact first, then the longest id the name starts with: a first party id
/// carries a date (`claude-haiku-4-5-20251001`) that the catalog key does not.
pub fn model_of(id: &str) -> Option<&'static Model> {
    catalog()
        .iter()
        .filter(|model| model.id == id || id.starts_with(&model.id))
        .max_by_key(|model| model.id.len())
}

/// The models the menu offers: the newest of each family, exactly the four
/// Claude Code puts in its own picker.
///
/// Newest is the largest id in the catalog for that family. The version sits
/// at the end of the name, so the order of the names is the order of the
/// versions, and a new model arrives in the menu as soon as the catalog is
/// captured again.
pub fn choices() -> Vec<ModelChoice> {
    FAMILIES
        .iter()
        .filter_map(|family| {
            let model = newest_of(family)?;
            Some(ModelChoice {
                alias: (*family).to_string(),
                label: model.label.clone(),
                id: model.id.clone(),
            })
        })
        .collect()
}

impl Effort {
    pub const ALL: [Effort; 5] = [
        Effort::Low,
        Effort::Medium,
        Effort::High,
        Effort::XHigh,
        Effort::Max,
    ];

    /// What travels in `/effort <level>`, the list of `claude --effort`.
    pub fn flag(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
            Effort::Max => "max",
        }
    }

    /// What the row says.
    pub fn label(self) -> &'static str {
        match self {
            Effort::Low => "Low",
            Effort::Medium => "Medium",
            Effort::High => "High",
            Effort::XHigh => "XHigh",
            Effort::Max => "Max",
        }
    }

    /// What the transcript wrote. Anything else is not an effort we know, and
    /// a level we cannot name is better left unsaid than guessed at.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|level| level.flag() == raw)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("that is not a model name")]
    BadModel,
}

/// The line that changes the model of a session.
///
/// The name is checked rather than escaped. A pty is a stream: a newline in
/// the argument would not be a mangled command, it would be a second line
/// submitted to the agent as a message, which is the one thing a setting must
/// never turn into. tech.md 6.15.
pub fn model_command(model: &str) -> Result<String, CommandError> {
    let model = model.trim();
    let sane = !model.is_empty()
        && model.len() <= 64
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'));

    if !sane {
        return Err(CommandError::BadModel);
    }
    Ok(format!("/model {model}"))
}

/// The line that changes the effort. The level comes from a closed set, so
/// there is nothing here to check.
pub fn effort_command(effort: Effort) -> String {
    format!("/effort {}", effort.flag())
}

/// The line the context ring sends when it is clicked.
pub const COMPACT_COMMAND: &str = "/compact";

/// What a session's context is measured against.
///
/// The catalog gives the model's window. What the catalog cannot know is the
/// account: a million is not given to everyone, and the entitlement that
/// decides it is not readable from outside Claude Code. So one correction is
/// applied, and only on evidence: an automatic compact that fired well below
/// the catalogue window proves the window was smaller than the catalog says,
/// because Claude Code compacts near the limit and nowhere else. tech.md 6.15.
pub fn window_for(model: Option<&str>, auto_compact_at: Option<u32>) -> u32 {
    let window = model
        .and_then(model_of)
        .map(|model| model.context_window)
        .unwrap_or(DEFAULT_WINDOW)
        .max(1);

    match auto_compact_at {
        Some(pre) if pre < window / 2 => step(pre).min(window),
        _ => window,
    }
}

/// Up to the next hundred thousand, and never below one: a compact at 4k
/// tokens is a compact somebody asked for, not the shape of a window.
fn step(tokens: u32) -> u32 {
    const STEP: u32 = 100_000;
    tokens.div_ceil(STEP).max(1).saturating_mul(STEP)
}

/// Where Claude Code keeps the model and the effort a new session starts with.
pub fn settings_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|home| {
        std::path::Path::new(&home)
            .join(".claude")
            .join("settings.json")
    })
}

/// What a session that has not answered yet is running as.
///
/// Claude Code saves every `/model` and `/effort` here as the default for new
/// sessions, and the island passes no `--model`, so this is what the session
/// it just started is actually using. A missing file or a missing key names
/// nothing rather than guessing: the picker is for choosing a model, not for
/// confirming one. tech.md 6.15.
pub fn defaults_from_settings(settings: &str) -> AgentSetup {
    let parsed: serde_json::Value =
        serde_json::from_str(settings).unwrap_or(serde_json::Value::Null);
    let alias = parsed
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|alias| !alias.is_empty());
    // The key holds what `/model` took: an alias like `opus`, or a full id.
    let known = alias.and_then(|alias| model_of(alias).or_else(|| newest_of(alias)));
    let effort = parsed
        .get("effortLevel")
        .and_then(serde_json::Value::as_str);

    AgentSetup::new(
        known
            .map(|model| model.id.clone())
            .or(alias.map(str::to_string)),
        known.map(|model| model.label.clone()),
        effort
            .and_then(Effort::parse)
            .filter(|_| known.is_none_or(Model::takes_effort)),
        known.map(Model::levels).unwrap_or_default(),
        // Nothing has been asked of the window yet, and the ring says so by
        // standing empty rather than by drawing a zero. tech.md 6.15.
        0,
        known
            .map(|model| model.context_window)
            .unwrap_or(DEFAULT_WINDOW),
    )
}

/// The newest model of a family, which is what an alias like `opus` means.
fn newest_of(family: &str) -> Option<&'static Model> {
    catalog()
        .iter()
        .filter(|model| model.family == family)
        .max_by(|left, right| left.id.cmp(&right.id))
}

/// Everything the row shows, from what the transcript said.
pub fn setup(
    model: Option<&str>,
    effort: Option<&str>,
    context_tokens: u32,
    auto_compact_at: Option<u32>,
) -> AgentSetup {
    let known = model.and_then(model_of);
    // A model that takes no effort has none to report, whatever a stale field
    // in the file says.
    let effort = effort
        .and_then(Effort::parse)
        .filter(|_| known.is_none_or(Model::takes_effort));

    AgentSetup::new(
        model.map(str::to_string),
        known.map(|model| model.label.clone()),
        effort,
        known.map(Model::levels).unwrap_or_default(),
        context_tokens,
        window_for(model, auto_compact_at),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_captured_catalog_parses() {
        assert!(catalog().len() >= 4, "the catalog is the table of 6.15");
        assert!(catalog().iter().all(|model| model.context_window > 0));
    }

    #[test]
    fn a_model_is_named_by_the_catalog() {
        let model = model_of("claude-opus-5").expect("opus 5 is in the catalog");
        assert_eq!(model.label, "Opus 5");
        assert_eq!(model.context_window, 1_000_000);
    }

    /// A first party id carries a date the catalog key does not.
    #[test]
    fn a_dated_id_finds_its_model() {
        let model = model_of("claude-haiku-4-5-20251001").expect("haiku 4.5 by prefix");
        assert_eq!(model.label, "Haiku 4.5");
    }

    #[test]
    fn the_menu_offers_the_newest_of_each_family() {
        let labels: Vec<String> = choices().into_iter().map(|choice| choice.label).collect();
        assert_eq!(labels, vec!["Fable 5", "Opus 5", "Sonnet 5", "Haiku 4.5"]);
        assert_eq!(
            choices().first().map(|choice| choice.alias.clone()),
            Some("fable".to_string())
        );
    }

    #[test]
    fn a_model_without_effort_offers_none() {
        let haiku = model_of("claude-haiku-4-5").expect("in the catalog");
        assert!(haiku.levels().is_empty());

        let opus = model_of("claude-opus-5").expect("in the catalog");
        assert_eq!(opus.levels(), Effort::ALL.to_vec());
    }

    #[test]
    fn every_level_survives_being_written_and_read() {
        for level in Effort::ALL {
            assert_eq!(Effort::parse(level.flag()), Some(level));
            assert_eq!(Effort::parse(&level.flag().to_uppercase()), Some(level));
        }
        assert_eq!(Effort::parse("enormous"), None);
    }

    #[test]
    fn the_commands_are_one_line_with_one_argument() {
        assert_eq!(model_command("opus").as_deref(), Ok("/model opus"));
        assert_eq!(effort_command(Effort::XHigh), "/effort xhigh");
        assert_eq!(COMPACT_COMMAND, "/compact");
    }

    /// A newline in the argument would not be a mangled command, it would be a
    /// message sent to the agent.
    #[test]
    fn a_model_name_that_could_carry_a_message_is_refused() {
        for name in [
            "opus\ndelete everything",
            "opus and then some",
            "",
            "   ",
            "opus;rm -rf /",
            "`opus`",
        ] {
            assert_eq!(model_command(name), Err(CommandError::BadModel), "{name}");
        }
    }

    #[test]
    fn an_unknown_model_measures_against_the_default() {
        assert_eq!(window_for(Some("claude-tomorrow-9"), None), DEFAULT_WINDOW);
        assert_eq!(window_for(None, None), DEFAULT_WINDOW);
    }

    /// The one thing the catalog cannot know is the account, and an automatic
    /// compact well below the catalogue window is the evidence of it.
    #[test]
    fn an_early_automatic_compact_lowers_the_window() {
        assert_eq!(window_for(Some("claude-opus-5"), Some(180_000)), 200_000);
        // Near the catalogue window it confirms rather than corrects.
        assert_eq!(
            window_for(Some("claude-opus-5"), Some(940_000)),
            1_000_000,
            "a compact at the limit says the limit is the limit"
        );
    }

    #[test]
    fn the_correction_never_grows_the_window() {
        for pre in [0, 1, 90_000, 199_999, 400_000, u32::MAX] {
            let window = window_for(Some("claude-haiku-4-5"), Some(pre));
            assert!(window <= 200_000, "grew to {window} on {pre}");
            assert!(window > 0);
        }
    }

    /// The alias Claude Code saves is what `/model` took, and it names the
    /// newest model of that family, exactly as the CLI resolves it.
    #[test]
    fn the_defaults_name_the_model_a_new_session_starts_with() {
        let setup = defaults_from_settings(r#"{"model": "opus", "effortLevel": "low"}"#);
        assert_eq!(setup.label.as_deref(), Some("Opus 5"));
        assert_eq!(setup.effort, Some(Effort::Low));
        assert_eq!(setup.context_window, 1_000_000);
        // Nothing has been asked of the window yet.
        assert_eq!(setup.context_tokens, 0);
    }

    #[test]
    fn a_full_id_in_the_settings_is_named_too() {
        let setup = defaults_from_settings(r#"{"model": "claude-haiku-4-5"}"#);
        assert_eq!(setup.label.as_deref(), Some("Haiku 4.5"));
        assert_eq!(setup.effort, None);
        assert!(setup.levels.is_empty());
    }

    /// A file that is missing, empty or silent about the model names nothing.
    /// The picker still works: it is for choosing one, not confirming one.
    #[test]
    fn settings_that_say_nothing_claim_nothing() {
        for settings in ["", "{}", "not json at all", r#"{"model": "   "}"#] {
            let setup = defaults_from_settings(settings);
            assert_eq!(setup.model, None, "{settings}");
            assert_eq!(setup.label, None);
            assert_eq!(setup.context_window, DEFAULT_WINDOW);
            assert_eq!(setup.context_pct, 0.0);
        }
    }

    #[test]
    fn a_reading_is_a_percentage_of_its_window() {
        let setup = setup(Some("claude-opus-5"), Some("high"), 250_000, None);
        assert_eq!(setup.label.as_deref(), Some("Opus 5"));
        assert_eq!(setup.effort, Some(Effort::High));
        assert_eq!(setup.context_window, 1_000_000);
        assert_eq!(setup.context_pct, 25.0);
    }

    #[test]
    fn a_model_that_takes_no_effort_reports_none() {
        let setup = setup(Some("claude-haiku-4-5"), Some("high"), 1_000, None);
        assert_eq!(setup.effort, None);
    }

    /// The user pinned their own window, and the reading follows it.
    #[test]
    fn a_pinned_window_is_the_one_that_counts() {
        let pinned = setup(Some("claude-opus-5"), None, 100_000, None).with_window(200_000);
        assert_eq!(pinned.context_window, 200_000);
        assert_eq!(pinned.context_pct, 50.0);
    }
}
