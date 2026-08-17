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
#
# Then drive a Claude Code session in another terminal: let a turn finish
# (Stop), trigger a tool that asks permission (PermissionRequest), let it write
# a todo list (PostToolUse/TodoWrite).

set -euo pipefail

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
echo "backed up settings to $BACKUP"

restore() {
  [ -f "$BACKUP" ] && cp "$BACKUP" "$SETTINGS"
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null || true
  echo "settings restored"
}
trap restore EXIT INT TERM

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

handler() { jq -n --arg url "http://127.0.0.1:$PORT/$1" '[{hooks: [{type: "http", url: $url, timeout: 10}]}]'; }

jq \
  --argjson stop "$(handler stop)" \
  --argjson permission "$(handler permission)" \
  --argjson notification "$(handler notification)" \
  --argjson tasks "$(handler tasks)" \
  --argjson session "$(handler session)" \
  '.hooks.Stop = $stop
   | .hooks.PermissionRequest = $permission
   | .hooks.Notification = $notification
   | .hooks.PostToolUse = $tasks
   | .hooks.SessionStart = $session
   | .hooks.SessionEnd = $session' \
  "$BACKUP" > "$SETTINGS"

echo "capturing for ${DURATION}s into $OUT"
echo "drive a Claude Code session now: finish a turn, trigger a permission, write a todo list"
sleep "$DURATION"

echo "captured files:"
ls -la "$OUT"
