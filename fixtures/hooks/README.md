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
- `session.jsonl` — `SessionEnd`

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
