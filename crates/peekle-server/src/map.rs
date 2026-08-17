//! Hook payload in, `PromptRequest` out. Outcome in, response body out.
//! tech.md section 6.2 holds every shape in this file.

use peekle_core::types::{
    ChoiceKind, ChoiceOption, PromptAnswer, PromptKind, PromptOutcome, PromptRequest, SessionRef,
};
use serde_json::{json, Value};

/// Reason Peekle sends back when the user picks continue.
pub const CONTINUE_REASON: &str = "Continue with the current plan.";

const LAST_MESSAGE_LIMIT: usize = 2000;
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
    }
}

/// Truncates on a character boundary and marks the cut, so the panel never
/// shows a silently shortened message.
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{kept}…")
}

pub fn stop_request(id: String, payload: &Value, now_ms: i64, expires_ms: i64) -> PromptRequest {
    PromptRequest {
        id,
        kind: PromptKind::Stop,
        session: session_ref(payload),
        title: "Claude finished".to_string(),
        last_message: string_field(payload, "last_assistant_message")
            .map(|text| truncate(&text, LAST_MESSAGE_LIMIT)),
        detail: None,
        options: vec![
            ChoiceOption {
                id: "continue".to_string(),
                label: "Continue".to_string(),
                hint: None,
                kind: ChoiceKind::Continue,
            },
            ChoiceOption {
                id: "finish".to_string(),
                label: "Finish".to_string(),
                hint: None,
                kind: ChoiceKind::Finish,
            },
        ],
        allow_free_text: true,
        created_at: now_ms,
        expires_at: expires_ms,
    }
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

/// Response body for `/stop`. An empty object ends the turn normally.
pub fn stop_body(outcome: &PromptOutcome, request: &PromptRequest) -> Value {
    let PromptOutcome::Answered(answer) = outcome else {
        return json!({});
    };

    match choice_kind(request, answer) {
        Some(ChoiceKind::Finish) => json!({}),
        Some(ChoiceKind::Continue) => json!({"decision": "block", "reason": CONTINUE_REASON}),
        // Free text is the next prompt the agent acts on, so it goes back
        // verbatim, trimmed and with no wrapper prose.
        _ => match answer
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            Some(text) => json!({"decision": "block", "reason": text}),
            None => json!({}),
        },
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

    fn stop() -> PromptRequest {
        stop_request("p".into(), &json!({"cwd": "/Users/x/peekle"}), 0, 0)
    }

    fn permission() -> PromptRequest {
        permission_request("p".into(), &json!({"tool_name": "Bash"}), 0, 0)
    }

    #[test]
    fn project_is_the_last_path_segment() {
        assert_eq!(stop().session.project, "peekle");
    }

    #[test]
    fn free_text_becomes_the_next_prompt_verbatim() {
        let body = stop_body(&answered(None, Some("  run the tests  ")), &stop());
        assert_eq!(
            body,
            json!({"decision": "block", "reason": "run the tests"})
        );
    }

    #[test]
    fn finish_ends_the_turn() {
        assert_eq!(
            stop_body(&answered(Some("finish"), None), &stop()),
            json!({})
        );
    }

    #[test]
    fn continue_sends_the_fixed_reason() {
        assert_eq!(
            stop_body(&answered(Some("continue"), None), &stop()),
            json!({"decision": "block", "reason": CONTINUE_REASON})
        );
    }

    #[test]
    fn every_non_answer_outcome_ends_the_turn_normally() {
        for outcome in [
            PromptOutcome::Dismissed,
            PromptOutcome::TimedOut,
            PromptOutcome::Bypassed,
        ] {
            assert_eq!(stop_body(&outcome, &stop()), json!({}));
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
        let request = stop_request("p".into(), &json!({"last_assistant_message": long}), 0, 0);
        let text = request.last_message.unwrap();
        assert_eq!(text.chars().count(), LAST_MESSAGE_LIMIT);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn missing_fields_do_not_break_the_prompt() {
        let request = stop_request("p".into(), &json!({}), 0, 0);
        assert_eq!(request.session.session_id, "");
        assert_eq!(request.last_message, None);
    }
}
