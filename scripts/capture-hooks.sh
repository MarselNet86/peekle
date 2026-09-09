#!/usr/bin/env bash
# Captures real Claude Code hook payloads into fixtures/hooks/.
#
# tech.md rule 6: fixtures are captured, never written by hand. A payload
# invented from documentation and drifting from reality is worse than no test.
#
# The script installs capture handlers into ~/.claude/settings.json, backs the
# file up first, and always restores it, including on Ctrl-C.
#
# Usage:
#   scripts/capture-hooks.sh [seconds]
#   scripts/capture-hooks.sh --restore   put settings back after a hard kill
#
# The trap covers a normal exit and Ctrl-C. It cannot cover SIGKILL, and a
# session left pointing at a dead capture server is a bad way to find that out,
# so the backup path is written to a marker file that --restore reads.
#
# Then drive a Claude Code session in another terminal: send a prompt
# (UserPromptSubmit), let it run tools (PreToolUse and PostToolUse), let a turn
# finish (Stop), trigger a tool that asks permission (PermissionRequest).

set -euo pipefail

MARKER=/tmp/peekle-capture-restore

if [ "${1:-}" = "--restore" ]; then
  [ -f "$MARKER" ] || { echo "nothing to restore" >&2; exit 1; }
  backup="$(cat "$MARKER")"
  [ -f "$backup" ] || { echo "backup $backup is gone" >&2; exit 1; }
  cp "$backup" "$HOME/.claude/settings.json"
  rm -f "$MARKER"
  echo "settings restored from $backup"
  exit 0
fi

PORT="${PEEKLE_CAPTURE_PORT:-47822}"
DURATION="${1:-180}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/fixtures/hooks"
SETTINGS="$HOME/.claude/settings.json"
BACKUP="$SETTINGS.peekle-bak.$(date +%s)"

command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
command -v node >/dev/null || { echo "node is required" >&2; exit 1; }

mkdir -p "$OUT" "$(dirname "$SETTINGS")"
[ -f "$SETTINGS" ] || echo '{}' > "$SETTINGS"
cp "$SETTINGS" "$BACKUP"
echo "$BACKUP" > "$MARKER"
echo "backed up settings to $BACKUP"

restore() {
  [ -f "$BACKUP" ] && cp "$BACKUP" "$SETTINGS"
  rm -f "$MARKER"
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null || true
  echo "settings restored"
}
trap restore EXIT INT TERM HUP

cat > /tmp/peekle-capture.mjs <<'NODE'
import { createServer } from 'node:http';
import { appendFileSync, mkdirSync } from 'node:fs';

const [, , port, out] = process.argv;
mkdirSync(out, { recursive: true });

createServer((req, res) => {
  const chunks = [];
  req.on('data', (c) => chunks.push(c));
  req.on('end', () => {
    const name = req.url.split('/').filter(Boolean).pop() ?? 'unknown';
    const body = Buffer.concat(chunks).toString('utf8');
    try {
      // One JSON object per line, exactly as it arrived.
      appendFileSync(`${out}/${name}.jsonl`, `${JSON.stringify(JSON.parse(body))}\n`);
      console.log(`captured ${name}`);
    } catch {
      console.error(`skipped a non-JSON body on ${name}`);
    }
    // Never block the agent while capturing.
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end('{}');
  });
}).listen(Number(port), '127.0.0.1', () => console.log(`capture server on ${port}`));
NODE

node /tmp/peekle-capture.mjs "$PORT" "$OUT" &
SERVER_PID=$!
sleep 1

# Mirrors the matcher column of tech.md 6.1. Without them the capture records
# every tool call rather than what the endpoint actually receives.
handler() {
  if [ -n "${2:-}" ]; then
    jq -n --arg url "http://127.0.0.1:$PORT/$1" --arg matcher "$2" \
      '[{matcher: $matcher, hooks: [{type: "http", url: $url, timeout: 10}]}]'
  else
    jq -n --arg url "http://127.0.0.1:$PORT/$1" \
      '[{hooks: [{type: "http", url: $url, timeout: 10}]}]'
  fi
}

# The feed endpoint takes three events, so each lands in its own file rather
# than all three in feed.jsonl: a golden test needs to name what it replays.
jq \
  --argjson stop "$(handler stop)" \
  --argjson permission "$(handler permission '*')" \
  --argjson notification "$(handler notification 'permission_prompt|idle_prompt|agent_needs_input|agent_completed')" \
  --argjson prompt "$(handler user_prompt_submit)" \
  --argjson pre "$(handler pre_tool_use '*')" \
  --argjson post "$(handler post_tool_use '*')" \
  --argjson session "$(handler session)" \
  '.hooks.Stop = $stop
   | .hooks.PermissionRequest = $permission
   | .hooks.Notification = $notification
   | .hooks.UserPromptSubmit = $prompt
   | .hooks.PreToolUse = $pre
   | .hooks.PostToolUse = $post
   | .hooks.SessionStart = $session
   | .hooks.SessionEnd = $session' \
  "$BACKUP" > "$SETTINGS"

echo "capturing for ${DURATION}s into $OUT"
echo "drive a Claude Code session now: send a prompt, let it run tools, finish a turn"
sleep "$DURATION"

# The capture came off a real machine and carries it: the home path, the
# account name in an `ls` column, an address in a terminal dump. The
# repository is public, so a capture is scrubbed before it is a fixture.
# tech.md rule 6.
"$ROOT/scripts/scrub-fixtures.py" "$OUT"/*.jsonl

echo "captured files:"
ls -la "$OUT"
