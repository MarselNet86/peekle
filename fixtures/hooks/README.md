# Captured hook payloads

Every file here comes off a live Claude Code session through
`scripts/capture-hooks.sh`. None of it is written by hand.

tech.md rule 6 is blunt about why: a payload invented from documentation that
drifts from what Claude Code actually sends is worse than having no test at
all. It passes, it looks like coverage, and it is wrong in exactly the place
the contract matters.

Format is one JSON object per line, `<endpoint>.jsonl`, exactly as it arrived
on the wire.

## Captured

- `stop.jsonl` — a turn finishing, carrying `last_assistant_message`
- `user_prompt_submit.jsonl` — a user turn, carrying `prompt`
- `pre_tool_use.jsonl` — tool calls opening, carrying `tool_use_id`
- `post_tool_use.jsonl` — tool calls closing, by the same `tool_use_id`
- `permission.jsonl` — `Bash` asking to append to a file
- `notification.jsonl` — an `idle_prompt`
- `tasks.jsonl` — `PostToolUse` for `TodoWrite`, with `tool_input.todos`
- `session.jsonl` — `SessionEnd`, six of them and not one `SessionStart`

`SessionStart` has never appeared in a capture, across every session driven
here, while its handler was installed the whole time. Nothing depends on it:
the registry opens a card on the first feed event, which is the one that
actually arrives.

## Capturing the two that need a real prompt

`permission.jsonl` does not fire in a headless `claude -p` run and it does not
fire under a permission mode that auto-approves. It also does not fire for a
command Claude Code reads as safe: `cat note.txt` is allowed without asking, so
the capture needs a command that writes.

The recipe that worked, driving an interactive session through a pty:

1. `./scripts/capture-hooks.sh 420`
2. `expect` spawns `claude --permission-mode manual` in a scratch directory,
   answers the trust and renderer dialogs, waits for the status line, then
   types `use the Bash tool to run: echo appended >> note.txt`
3. The permission prompt appears, the hook fires, the capture records it

Match the TUI on single words. Claude Code writes cursor positioning escapes
between them, so `for shortcuts` never appears contiguously in the stream and
a two word pattern never matches.

## When a capture disagrees with tech.md

The capture wins. Fix section 6, bump the core version, and land the contract
change before the code that depends on it. tech.md section 13.

## `ask_user_question.jsonl`

`AskUserQuestion` arrives through `/v1/h/{token}/permission` in this environment
(`hook_event_name: "PermissionRequest"`, `tool_name: "AskUserQuestion"`), not
through `PreToolUse` the way the plain `claude` CLI documents it. Captured live
by asking a real, necessary question through this exact tool while
`capture-hooks.sh` was running — `tool_input.questions` is the genuine wire
shape: one to four questions, each with `question`, `header`, `options`
(`label` plus optional `description`), and `multiSelect`.

Answering it does not go through `decision.behavior` alone. Per
[code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks),
`PermissionRequest` also accepts `updatedInput` inside `decision`: echo the
`questions` array back verbatim and add an `answers` object mapping each
question's text to the chosen label (multi-select answers joined with a
comma). `"allow"` alone does not answer the tool, only `updatedInput` does.

## `user_prompt_submit_peer.jsonl`

`UserPromptSubmit` as it fires in a process that took a message through its
inbox (tech.md 6.5): the prompt arrives wrapped in `<cross-session-message>`
with `from`, `from-name` and `from-mode` attributes, and the words a person
typed are the body. Captured 2026-09-08 off a headless `claude` 2.1.261
(`-p --input-format stream-json`) started in a scratch directory with a
command hook that wrote its stdin to a log, then sent one message over the
socket from another Claude session. The line is the payload the hook read,
untouched.
