# Captured hook payloads

Every file here comes off a live Claude Code session through
`scripts/capture-hooks.sh`. None of it is written by hand.

tech.md rule 6 is blunt about why: a payload invented from documentation that
drifts from what Claude Code actually sends is worse than having no test at
all. It passes, it looks like coverage, and it is wrong in exactly the place
the contract matters.

Format is one JSON object per line, `<endpoint>.jsonl`, exactly as it arrived
on the wire.

## What to capture before the framework is done

- `stop.jsonl` — a turn finishing
- `permission.jsonl` — a tool asking for permission, ideally `Bash`
- `notification.jsonl` — an idle or input prompt
- `tasks.jsonl` — `PostToolUse` for `TodoWrite`
- `session.jsonl` — `SessionStart` and `SessionEnd`

## When a capture disagrees with tech.md

The capture wins. Fix section 6, bump the core version, and land the contract
change before the code that depends on it. tech.md section 13.
