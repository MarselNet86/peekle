#!/usr/bin/env bash
# Checks the first line of the tech.md section 15 checklist: does the island
# appear over another application's full screen space.
#
# This was R-11, and it stayed open for a while because the obvious way to test
# it is to open a video and look. That is how the first investigation went
# wrong: the video left full screen midway through a sweep of window levels and
# the sweep appeared to find a fix that did not exist. So this drives its own
# full screen window and verifies the space is still active on both sides of
# the measurement.
#
#   scripts/macos/check-fullscreen.sh [path to Peekle.app]

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP="${1:-$ROOT/target/debug/bundle/macos/Peekle.app}"
HERE="$ROOT/scripts/macos"
DOC="${TMPDIR:-/tmp}/peekle-fullscreen-probe.txt"
PORT="${PEEKLE_PORT:-47821}"
TOKEN="${PEEKLE_TOKEN:-0123456789abcdef0123456789abcdef}"

[ -d "$APP" ] || { echo "no app at $APP, run: pnpm tauri build --debug" >&2; exit 2; }

full_screen_active() {
  swift "$HERE/onscreen.swift" 2>/dev/null | grep -q "full_screen_space=true"
}

cleanup() {
  osascript -e 'tell application "TextEdit" to quit saving no' >/dev/null 2>&1 || true
  pkill -f 'Peekle.app' 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "full screen probe" > "$DOC"
pkill -f 'Peekle.app' 2>/dev/null; sleep 1

osascript -e 'tell application "TextEdit" to close every window saving no' >/dev/null 2>&1 || true
open -a TextEdit "$DOC"; sleep 2
osascript -e 'tell application "TextEdit" to activate' >/dev/null 2>&1; sleep 1
osascript -e 'tell application "System Events" to tell process "TextEdit" to set value of attribute "AXFullScreen" of window 1 to true' >/dev/null 2>&1
sleep 5

full_screen_active || {
  echo "INCONCLUSIVE: could not put TextEdit into a full screen space"
  echo "System Settings > Privacy & Security > Accessibility has to allow the terminal"
  exit 3
}

open -g -a "$APP" --stdout /dev/null --stderr /dev/null
sleep 4
curl -sS -m 5 -X POST "http://127.0.0.1:$PORT/v1/h/$TOKEN/notification" \
  -H 'content-type: application/json' \
  -d '{"hook_event_name":"Notification","message":"full screen check"}' >/dev/null 2>&1
sleep 1

seen=$(swift "$HERE/onscreen.swift" Peekle 2>/dev/null | grep -c '^Peekle')

if ! full_screen_active; then
  echo "INCONCLUSIVE: the full screen space went away during the check"
  exit 3
fi

if [ "$seen" -gt 0 ]; then
  echo "PASS: the island is on the full screen space"
  exit 0
fi
echo "FAIL: the island is not on the full screen space"
exit 1
