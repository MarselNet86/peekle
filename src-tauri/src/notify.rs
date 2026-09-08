//! The banner a finished turn puts on the screen. tech.md 6.17.
//!
//! The pill of 6.2 reaches whoever is already looking at the screen: it lives
//! `TURN_NOTICE_MS` and only rises on a resting island. Someone who started an
//! agent and walked off into another app learns the turn ended by coming back,
//! which is the context switch this product exists to remove. So the end of a
//! turn has a second channel, and it is the system's own.
//!
//! What decides whether to post is a free function, so the tests can hammer it
//! without a window, a plugin or a screen.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// What every banner says on top. It names the state the person is in -- their
/// turn to look -- rather than the event, because by the time they read it the
/// event is over. tech.md 6.17.
pub const TITLE: &str = "Claude is waiting for you";

/// What the banner says when the toggle itself is switched on.
///
/// It is the first post, and the first post is what makes macOS ask. So it is
/// also the answer to "did that do anything": the person sees the banner they
/// just signed up for, in the place it will appear from now on. tech.md 6.17.
pub const SWITCHED_ON: &str = "Turn notices are on. This is what one looks like.";

/// The body of the banner a finished turn earns, or nothing at all.
///
/// Three ways to earn nothing, and each is a different statement. The toggle is
/// off: the person did not ask, and the system is not touched. The island is
/// already open on this very session: they are looking at it. The agent said
/// nothing: the pill stays quiet on an empty `last_assistant_message` too, and
/// a banner with a project name and no words answers no question.
///
/// The project leads, the way it does in the pill: more than one session runs
/// at a time, and which one finished is the first thing worth knowing.
/// tech.md 6.17.
pub fn notice(enabled: bool, watching: bool, project: &str, said: &str) -> Option<String> {
    if !enabled || watching {
        return None;
    }
    let line = crate::hooks::first_line(said);
    if line.is_empty() {
        return None;
    }
    Some(format!("{project} · {line}"))
}

/// Puts one banner on the screen.
///
/// A trait so the tests get a fake by the convention of section 7: nothing in
/// a test run reaches the system, and no test raises a permission dialog.
pub trait Notifier: Send + Sync {
    fn post(&self, title: &str, body: &str) -> Result<(), String>;
}

/// The real one. macOS raises its own permission dialog on the first post --
/// there is no separate request on desktop, `request_permission` of the plugin
/// answers `Granted` without asking anyone -- which is why the only thing that
/// posts before the toggle is pressed is nothing at all. tech.md 6.17.
pub struct SystemNotifier<'a, R: tauri::Runtime>(pub &'a AppHandle<R>);

impl<R: tauri::Runtime> Notifier for SystemNotifier<'_, R> {
    fn post(&self, title: &str, body: &str) -> Result<(), String> {
        self.0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|err| err.to_string())
    }
}

/// Posts the banner a finished turn earns, if it earns one.
pub fn say(app: &AppHandle, enabled: bool, watching: bool, project: &str, said: &str) {
    let Some(body) = notice(enabled, watching, project, said) else {
        return;
    };
    if let Err(err) = SystemNotifier(app).post(TITLE, &body) {
        // A banner that did not appear is not worth a screen of its own: the
        // island already said the same thing where the person can see it.
        tracing::warn!(error = %err, "the system refused a turn notice");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The fake of section 7: counts what it was asked to show and touches
    /// nothing. No test raises a permission dialog.
    #[derive(Default)]
    struct FakeNotifier(Mutex<Vec<(String, String)>>);

    impl Notifier for FakeNotifier {
        fn post(&self, title: &str, body: &str) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push((title.to_string(), body.to_string()));
            Ok(())
        }
    }

    #[test]
    fn the_title_is_the_one_the_contract_names() {
        assert_eq!(TITLE, "Claude is waiting for you");
    }

    /// The project leads and the words follow, the same line the pill carries.
    #[test]
    fn the_body_names_the_project_and_what_was_said() {
        assert_eq!(
            notice(true, false, "peekle", "Done. Two files changed."),
            Some("peekle · Done. Two files changed.".to_string())
        );
    }

    /// A reply is prose and prose has paragraphs. One line is what a banner
    /// holds, and it is enough to decide whether to go and look.
    #[test]
    fn only_the_first_line_travels() {
        let body = notice(true, false, "peekle", "\n\nShipped it.\nAnd then some.").unwrap();
        assert_eq!(body, "peekle · Shipped it.");
        assert!(!body.contains('\n'));
    }

    /// Three silences, three different reasons. tech.md 6.17.
    #[test]
    fn nothing_is_posted_unasked_unread_or_unsaid() {
        assert_eq!(
            notice(false, false, "peekle", "Done."),
            None,
            "toggle is off"
        );
        assert_eq!(notice(true, true, "peekle", "Done."), None, "already open");
        assert_eq!(
            notice(true, false, "peekle", "   \n\n"),
            None,
            "said nothing"
        );
    }

    proptest::proptest! {
        /// A reply is whatever the agent wrote, and the decision runs on every
        /// end of every turn. It has to be total over any text, and it has to
        /// keep both silences whatever the words are: the person who did not
        /// ask never gets a banner, and the person already looking at that
        /// session never gets one either. tech.md 6.17.
        #[test]
        fn the_decision_is_total_and_keeps_both_silences(
            said in ".*",
            project in ".*",
            watching in proptest::bool::ANY,
        ) {
            let _ = notice(true, watching, &project, &said);
            proptest::prop_assert!(notice(false, watching, &project, &said).is_none());
            proptest::prop_assert!(notice(true, true, &project, &said).is_none());
        }

        /// Whatever it does say fits a banner: one line, and the project in
        /// front of it. tech.md 6.17.
        #[test]
        fn whatever_it_says_is_one_line_behind_the_project(said in ".*") {
            if let Some(body) = notice(true, false, "peekle", &said) {
                proptest::prop_assert!(body.starts_with("peekle · "));
                proptest::prop_assert!(!body.contains('\n'));
            }
        }
    }

    /// The fake stands in for the system so a test run posts nothing. What it
    /// is handed is what a real post would carry.
    #[test]
    fn a_fake_notifier_carries_the_same_two_strings() {
        let fake = FakeNotifier::default();
        let body = notice(true, false, "peekle", "Done.").unwrap();
        fake.post(TITLE, &body).unwrap();

        let posted = fake.0.lock().unwrap().clone();
        assert_eq!(
            posted,
            vec![(TITLE.to_string(), "peekle · Done.".to_string())]
        );
    }
}
