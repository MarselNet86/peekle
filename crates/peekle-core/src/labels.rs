//! Task label classifier. Total, pure, and stable: the HUD renders whatever it
//! returns, so it never panics and never leaves a title unlabelled.

use crate::types::TaskLabel;

/// Keyword table, most specific first. `Bug` sits above `Fix` so that
/// "fix the crash" reads as a bug rather than as generic repair work.
const RULES: &[(TaskLabel, &[&str])] = &[
    (
        TaskLabel::Bug,
        &["bug", "regression", "crash", "broken", "breaks", "panic"],
    ),
    (
        TaskLabel::Test,
        &["test", "spec", "coverage", "fixture", "assert"],
    ),
    (
        TaskLabel::Docs,
        &["doc", "readme", "changelog", "comment", "guide"],
    ),
    (
        TaskLabel::Research,
        &[
            "research",
            "investigate",
            "explore",
            "spike",
            "compare",
            "evaluate",
        ],
    ),
    (
        TaskLabel::Fix,
        &["fix", "repair", "resolve", "patch", "correct"],
    ),
    (
        TaskLabel::Chore,
        &[
            "chore", "bump", "rename", "cleanup", "clean up", "lint", "format", "upgrade",
        ],
    ),
];

/// Classifies a task title. Anything unrecognised is `Code`.
pub fn classify(title: &str) -> TaskLabel {
    let haystack = title.to_lowercase();
    for (label, keywords) in RULES {
        if keywords.iter().any(|kw| haystack.contains(kw)) {
            return *label;
        }
    }
    TaskLabel::Code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_keywords() {
        assert_eq!(classify("Fix the crash on startup"), TaskLabel::Bug);
        assert_eq!(classify("Add tests for the parser"), TaskLabel::Test);
        assert_eq!(classify("Update README"), TaskLabel::Docs);
        assert_eq!(classify("Investigate slow startup"), TaskLabel::Research);
        assert_eq!(classify("Fix the off by one"), TaskLabel::Fix);
        assert_eq!(classify("Bump tauri to 2.10"), TaskLabel::Chore);
    }

    #[test]
    fn falls_back_to_code() {
        assert_eq!(classify("Ship the overlay"), TaskLabel::Code);
        assert_eq!(classify(""), TaskLabel::Code);
    }

    #[test]
    fn ignores_case() {
        assert_eq!(classify("REGRESSION in the hud"), TaskLabel::Bug);
    }
}
