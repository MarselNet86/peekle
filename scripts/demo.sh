#!/usr/bin/env bash
# Drives the island by hand, the way Claude Code would.
# Dev aid only: `peekle init` (S9) is what wires a real session.
#
#   scripts/demo.sh island   one toast, hides itself after 2.6s
#   scripts/demo.sh tasks    three tasks into the registry
#   scripts/demo.sh clear    empties the task list
#   scripts/demo.sh stop     blocks like a real Stop hook

set -euo pipefail

PORT="${PEEKLE_PORT:-47821}"
TOKEN="${PEEKLE_TOKEN:-0123456789abcdef0123456789abcdef}"
BASE="http://127.0.0.1:$PORT/v1/h/$TOKEN"

post() { curl -sS -X POST "$BASE/$1" -H 'content-type: application/json' -d "$2"; echo; }

case "${1:-island}" in
  island)
    post notification '{"hook_event_name":"Notification","message":"Claude needs your input"}'
    ;;
  tasks)
    post tasks '{"session_id":"demo","tool_input":{"todos":[
      {"content":"Fix the crash in the feed","status":"in_progress"},
      {"content":"Write release notes for v1","status":"pending"},
      {"content":"Investigate slow startup","status":"pending"}
    ]}}'
    ;;
  clear)
    post tasks '{"session_id":"demo","tool_input":{"todos":[]}}'
    ;;
  stop)
    echo "blocks until the timeout lapses: the island has no prompt UI before S3"
    post stop '{"hook_event_name":"Stop","session_id":"demo","cwd":"'"$PWD"'","last_assistant_message":"I finished the refactor and all tests pass. Want me to open a PR?"}'
    ;;
  health)
    curl -sS "http://127.0.0.1:$PORT/v1/health"; echo
    ;;
  *)
    echo "usage: $0 [island|tasks|clear|stop|health]" >&2
    exit 2
    ;;
esac
