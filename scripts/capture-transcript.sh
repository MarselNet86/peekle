#!/usr/bin/env bash
# Captures one Claude Code transcript as a fixture, with the conversation
# redacted. tech.md 6.11.
#
# Unlike a hook payload, a transcript carries the user's own conversation, so
# it cannot go into the repository as it stands. Every key, type, order and
# length survives; the text does not. The parser is checked against the shape,
# which is the part Claude Code can change under us.
#
#     scripts/capture-transcript.sh <session_id|path> [name]

set -euo pipefail

SOURCE="${1:?usage: capture-transcript.sh <session_id|path> [name]}"
NAME="${2:-session}"
OUT="$(cd "$(dirname "$0")/.." && pwd)/fixtures/transcripts/${NAME}.jsonl"

if [ ! -f "$SOURCE" ]; then
  SOURCE="$(find "$HOME/.claude/projects" -name "${SOURCE}*.jsonl" | head -1)"
fi
[ -f "$SOURCE" ] || { echo "no transcript found" >&2; exit 1; }

python3 - "$SOURCE" "$OUT" <<'PY'
import json, sys, re

src, out = sys.argv[1], sys.argv[2]

# Same length, no meaning. A shorter placeholder would hide a truncation bug.
def redact(text):
    return re.sub(r"[^\s]", "x", text)

KEEP = {"type", "role", "name"}

def walk(node, key=None):
    if isinstance(node, dict):
        return {k: walk(v, k) for k, v in node.items()}
    if isinstance(node, list):
        return [walk(v, key) for v in node]
    if isinstance(node, str) and key not in KEEP:
        # Ids, timestamps and paths stay: the parser reads them, and none of
        # them is the conversation.
        if key in {"sessionId", "uuid", "parentUuid", "timestamp", "cwd", "version",
                   "gitBranch", "leafUuid", "requestId", "id", "model", "promptId"}:
            return node
        return redact(node)
    return node

lines = 0
with open(src, encoding="utf-8") as source, open(out, "w", encoding="utf-8") as target:
    for line in source:
        line = line.strip()
        if not line:
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        target.write(json.dumps(walk(record), ensure_ascii=False) + "\n")
        lines += 1

print(f"{lines} records -> {out}")
PY
