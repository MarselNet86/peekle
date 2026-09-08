//! S20 acceptance for the parts that own a process. tech.md 6.16.
//!
//! Every end of a sign-in -- cancelled, exited, refused -- resolves the state
//! exactly once and leaves no process behind. Rule 10 is about hooks, but a
//! login that hangs half-resolved wedges the island the same way.

use std::sync::Arc;

use peekle_core::auth::SignInHost;
use peekle_core::types::SignInStage;

#[test]
fn a_fresh_host_is_idle_and_holds_nothing() {
    let host = SignInHost::new();
    assert_eq!(host.state().stage, SignInStage::Idle);
    assert_eq!(host.state().url, None);
    assert!(!host.state().needs_code);
}

/// A press on Cancel with nothing running is not an error: the process may
/// have exited a moment earlier, and refusing there would leave the panel
/// open over nothing.
#[test]
fn cancelling_nothing_is_not_an_error() {
    let host = SignInHost::new();
    host.cancel();
    host.cancel();
    assert_eq!(host.state().stage, SignInStage::Idle);
}

/// The code goes into a process's stdin. With no process there is nowhere to
/// put it, and it must be refused rather than swallowed -- a code that
/// vanishes silently is the same lie the reply field is built not to tell.
#[test]
fn a_code_with_nothing_to_receive_it_is_refused() {
    let host = SignInHost::new();
    assert!(host.submit_code("abc123").is_err());
    assert_eq!(host.state().stage, SignInStage::Idle);
}

/// An empty code is refused before anything is written: the CLI's own prompt
/// would take it as a bare return and answer nothing.
#[test]
fn an_empty_code_never_reaches_the_process() {
    let host = SignInHost::new();
    assert!(host.submit_code("   ").is_err());
    assert!(host.submit_code("").is_err());
}

/// Settling is the caller's answer to "did this work", and both answers are
/// terminal: neither leaves the panel waiting.
#[test]
fn settling_ends_the_run_either_way() {
    let host = SignInHost::new();

    let done = host.settle(true, None);
    assert_eq!(done.stage, SignInStage::Done);
    assert_eq!(done.error, None);
    assert!(!done.needs_code);

    let failed = host.settle(false, Some("that code was not accepted".into()));
    assert_eq!(failed.stage, SignInStage::Failed);
    assert_eq!(failed.error.as_deref(), Some("that code was not accepted"));
}

/// A binary that is not there fails at the spawn rather than leaving a host
/// that thinks something is running.
#[test]
fn a_missing_binary_leaves_nothing_running() {
    let host = Arc::new(SignInHost::new());
    let started = host.start(std::path::Path::new("/nonexistent/claude"), |_| {});

    assert!(started.is_err());
    assert!(
        host.submit_code("abc").is_err(),
        "a failed start must not leave a process to write to"
    );
}

/// Rule 11 covers the token; 6.16 extends it word for word to the code from
/// the authorize page and to the address itself, which carries
/// `code_challenge` and `state`. Checked in the source, because the failure
/// mode is a line that is written once and then never looked at again.
#[test]
fn neither_the_code_nor_the_address_is_ever_logged() {
    let sources = [
        include_str!("../src/auth.rs"),
        include_str!("../../../src-tauri/src/commands.rs"),
    ];

    for source in sources {
        for line in source.lines() {
            let line = line.trim();
            if !line.starts_with("tracing::") {
                continue;
            }
            // The message text is prose and may say the words; what counts
            // is the fields, so the quoted parts go first.
            let fields: String = line.split('"').step_by(2).collect();
            for field in fields.split(',') {
                if !field.contains("code") && !field.contains("url") {
                    continue;
                }
                // The permitted shapes, and the only ones: how long it is,
                // and whether there is one. Both are enough to debug with and
                // disclose nothing. tech.md 6.16 and rule 11.
                let value = field.trim();
                assert!(
                    value.ends_with(".len()") || value.ends_with(".is_some()"),
                    "a tracing field carries the code or the address itself: {line}"
                );
            }
        }
    }
}
