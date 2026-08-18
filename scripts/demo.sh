#!/usr/bin/env bash
# Drives the island by hand, the way Claude Code would.
# Dev aid only: `peekle init` (S9) is what wires a real session.
#
#   scripts/demo.sh island   one toast, hides itself after 2.6s
#   scripts/demo.sh feed     replays captured feed payloads
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
  feed)
    # Real captured payloads, in the order they arrived. tech.md rule 6.
    for file in user_prompt_submit pre_tool_use post_tool_use; do
      path="$(dirname "${BASH_SOURCE[0]}")/../fixtures/hooks/$file.jsonl"
      [ -f "$path" ] || { echo "capture $file.jsonl first" >&2; continue; }
      while read -r line; do
        [ -n "$line" ] && post feed "$line" >/dev/null
      done < "$path"
      echo "replayed $file"
    done
    ;;
  tasks)
    post feed '{"session_id":"demo","hook_event_name":"PostToolUse","tool_input":{"todos":[
      {"content":"Fix the crash in the feed","status":"in_progress"},
      {"content":"Write release notes for v1","status":"pending"},
      {"content":"Investigate slow startup","status":"pending"}
    ]}}'
    ;;
  clear)
    post feed '{"session_id":"demo","hook_event_name":"PostToolUse","tool_input":{"todos":[]}}'
    ;;
  stop)
    echo "blocks until the timeout lapses: the island has no prompt UI before S3"
    post stop '{"hook_event_name":"Stop","session_id":"demo","cwd":"'"$PWD"'","last_assistant_message":"I finished the refactor and all tests pass. Want me to open a PR?"}'
    ;;
  health)
    curl -sS "http://127.0.0.1:$PORT/v1/health"; echo
    ;;
  *)
    echo "usage: $0 [island|feed|tasks|clear|stop|health]" >&2
    exit 2
    ;;
esac
