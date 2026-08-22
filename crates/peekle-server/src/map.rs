//! Hook payload in, `PromptRequest` out. Outcome in, response body out.
//! tech.md section 6.2 holds every shape in this file.

use peekle_core::truncate;
use peekle_core::types::{
    ChoiceKind, ChoiceOption, PromptAnswer, PromptKind, PromptOutcome, PromptRequest, SessionRef,
};
use serde_json::{json, Value};

const DETAIL_LIMIT: usize = 400;

/// Claude Code adds fields between versions, so every lookup is optional and
/// a missing one degrades the panel rather than rejecting the hook.
fn string_field(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn session_ref(payload: &Value) -> SessionRef {
    let cwd = string_field(payload, "cwd").unwrap_or_default();
    let project = cwd
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("")
        .to_string();
    SessionRef {
        session_id: string_field(payload, "session_id").unwrap_or_default(),
        cwd,
        project,
        // Added by the hook script, which is a child of the agent. Absent for
        // anything that did not come through it. tech.md 6.1.
        pid: payload.get("pid").and_then(Value::as_u64).map(|p| p as u32),
        tty: string_field(payload, "tty"),
    }
}

/// Response body that hands the agent its next prompt. tech.md 6.2.
///
/// The text is what the user typed, so it goes back verbatim: reason is the
/// prompt the agent acts on, and wrapper prose would become an instruction.
pub fn block_body(text: &str) -> Value {
    json!({"decision": "block", "reason": text.trim()})
}

pub fn permission_request(
    id: String,
    payload: &Value,
    now_ms: i64,
    expires_ms: i64,
) -> PromptRequest {
    let tool = string_field(payload, "tool_name").unwrap_or_else(|| "a tool".to_string());
    let detail = payload.get("tool_input").map(|input| {
        let rendered = match input {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        truncate(&rendered, DETAIL_LIMIT)
    });

    PromptRequest {
        id,
        kind: PromptKind::Permission,
        session: session_ref(payload),
        title: format!("{tool} needs permission"),
        last_message: None,
        detail,
        options: vec![
            ChoiceOption {
                id: "allow_once".to_string(),
                label: "Allow once".to_string(),
                hint: None,
                kind: ChoiceKind::AllowOnce,
            },
            ChoiceOption {
                id: "allow_always".to_string(),
                label: "Allow for this session".to_string(),
                hint: None,
                kind: ChoiceKind::AllowAlways,
            },
            ChoiceOption {
                id: "deny".to_string(),
                label: "Deny".to_string(),
                hint: Some("Type a reason first".to_string()),
                kind: ChoiceKind::Deny,
            },
        ],
        allow_free_text: true,
        created_at: now_ms,
        expires_at: expires_ms,
    }
}

/// Response body for `/permission`.
pub fn permission_body(outcome: &PromptOutcome, request: &PromptRequest) -> Value {
    let PromptOutcome::Answered(answer) = outcome else {
        return json!({});
    };

    let decision = match choice_kind(request, answer) {
        Some(ChoiceKind::AllowOnce) | Some(ChoiceKind::AllowAlways) => json!({"behavior": "allow"}),
        Some(ChoiceKind::Deny) => {
            let mut deny = json!({"behavior": "deny"});
            if let Some(reason) = answer
                .text
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
            {
                deny["message"] = json!(reason);
            }
            deny
        }
        // No recognised choice means no decision. Claude Code falls back to
        // its own prompt, which is the safe direction to fail.
        _ => return json!({}),
    };

    json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": decision,
        }
    })
}

fn choice_kind(request: &PromptRequest, answer: &PromptAnswer) -> Option<ChoiceKind> {
    let id = answer.choice.as_deref()?;
    request
        .options
        .iter()
        .find(|option| option.id == id)
        .map(|option| option.kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answered(choice: Option<&str>, text: Option<&str>) -> PromptOutcome {
        PromptOutcome::Answered(PromptAnswer {
            prompt_id: "p".to_string(),
            choice: choice.map(str::to_owned),
            text: text.map(str::to_owned),
        })
    }

    fn permission() -> PromptRequest {
        permission_request("p".into(), &json!({"tool_name": "Bash"}), 0, 0)
    }

    #[test]
    fn project_is_the_last_path_segment() {
        assert_eq!(
            permission_request("p".into(), &json!({"cwd": "/Users/x/peekle"}), 0, 0)
                .session
                .project,
            "peekle"
        );
    }

    /// The queued text is the next prompt the agent acts on, so it goes back
    /// exactly as typed. Only the surrounding whitespace is ours to remove.
    #[test]
    fn queued_text_becomes_the_next_prompt_verbatim() {
        assert_eq!(
            block_body("  run the tests  "),
            json!({"decision": "block", "reason": "run the tests"})
        );
    }

    /// Prose around the text would read to the agent as instruction.
    #[test]
    fn nothing_is_wrapped_around_the_text() {
        for text in [
            "ship it",
            "rm -rf /tmp/x",
            "{\"json\": true}",
            "многострочный\nтекст",
        ] {
            assert_eq!(block_body(text)["reason"], json!(text));
        }
    }

    #[test]
    fn every_non_answer_outcome_ends_the_turn_normally() {
        for outcome in [
            PromptOutcome::Dismissed,
            PromptOutcome::TimedOut,
            PromptOutcome::Bypassed,
        ] {
            assert_eq!(permission_body(&outcome, &permission()), json!({}));
        }
    }

    #[test]
    fn allow_once_and_allow_always_both_allow() {
        for choice in ["allow_once", "allow_always"] {
            assert_eq!(
                permission_body(&answered(Some(choice), None), &permission()),
                json!({"hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {"behavior": "allow"}
                }})
            );
        }
    }

    #[test]
    fn deny_carries_the_typed_reason() {
        let body = permission_body(&answered(Some("deny"), Some("too broad")), &permission());
        assert_eq!(
            body["hookSpecificOutput"]["decision"],
            json!({"behavior": "deny", "message": "too broad"})
        );
    }

    #[test]
    fn deny_without_text_still_denies() {
        let body = permission_body(&answered(Some("deny"), None), &permission());
        assert_eq!(
            body["hookSpecificOutput"]["decision"],
            json!({"behavior": "deny"})
        );
    }

    #[test]
    fn long_messages_are_cut_on_a_character_boundary() {
        let long = "я".repeat(3000);
        let text = truncate(&long, peekle_core::LAST_MESSAGE_LIMIT);
        assert_eq!(text.chars().count(), peekle_core::LAST_MESSAGE_LIMIT);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn missing_fields_do_not_break_the_prompt() {
        let request = permission_request("p".into(), &json!({}), 0, 0);
        assert_eq!(request.session.session_id, "");
        assert_eq!(request.detail, None);
    }
}
