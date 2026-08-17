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
- `tasks.jsonl` — `PostToolUse` for `TodoWrite`, with `tool_input.todos`
- `session.jsonl` — `SessionEnd`

## Still missing

- `permission.jsonl` — a tool asking for permission, ideally `Bash`
- `notification.jsonl` — an idle or input prompt

Neither fires in a headless `claude -p` run: the permission prompt has no one
to answer it, so the tool is refused before the hook is reached. Both need an
interactive session. Start the capture, run `claude` in another terminal, and
approve or deny a tool by hand.

## When a capture disagrees with tech.md

The capture wins. Fix section 6, bump the core version, and land the contract
change before the code that depends on it. tech.md section 13.
