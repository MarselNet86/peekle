#!/usr/bin/env bash
# Captures Claude Code's own model catalog as a fixture. tech.md 6.15.
#
# The window a session's context is measured against is decided by the model,
# and the only place that number is written down is the `claude` binary itself.
# Guessing it means a ring that is wrong by a factor of five, so the table is
# taken from the source rather than typed out from memory.
#
#     scripts/capture-models.sh [path-to-claude]
#
# Re-run it after a Claude Code upgrade: a new model arrives with its own
# window, and an unknown model falls back to 200k the way the CLI itself does.

set -euo pipefail

BINARY="${1:-$(command -v claude)}"
[ -x "$BINARY" ] || { echo "no claude binary at ${BINARY}" >&2; exit 1; }
BINARY="$(readlink -f "$BINARY" 2>/dev/null || python3 -c 'import os,sys;print(os.path.realpath(sys.argv[1]))' "$BINARY")"
OUT="$(cd "$(dirname "$0")/.." && pwd)/fixtures/models/catalog.json"
mkdir -p "$(dirname "$OUT")"

python3 - "$BINARY" "$OUT" <<'PY'
import json, re, subprocess, sys

binary, out = sys.argv[1], sys.argv[2]
data = open(binary, "rb").read()

# One catalog entry, as the bundle writes it: the id, what the picker calls it,
# the context window, and the capabilities that decide whether it takes effort.
# Read one entry at a time from its own id: a span that reaches across the next
# id swallows that model whole, and the catalog comes back short.
head = re.compile(r'\{id:"(claude-[\w.\-]+)",family:"(\w+)",display_name:"([^"]+)"')

models = {}
text = data.decode("utf-8", "replace")
for match in head.finditer(text):
    model_id = match.group(1)
    if model_id in models:
        continue
    entry = text[match.end() : match.end() + 3000]
    window = re.search(r"context:\{window:([0-9e.]+)([^}]*)\}", entry)
    if window is None:
        continue
    caps = re.search(r"capabilities:\[([^\]]*)\]", entry)
    models[model_id] = {
        "id": model_id,
        "family": match.group(2),
        "label": match.group(3),
        "context_window": int(float(window.group(1))),
        "native_1m": "native_1m:!0" in window.group(2),
        "capabilities": re.findall(r'"([\w_]+)"', caps.group(1)) if caps else [],
    }

if not models:
    print("no catalog found in the binary", file=sys.stderr)
    raise SystemExit(1)

version = subprocess.run([binary, "--version"], capture_output=True, text=True).stdout.strip()
payload = {
    "captured_from": version,
    "models": [models[key] for key in sorted(models)],
}
with open(out, "w", encoding="utf-8") as target:
    json.dump(payload, target, indent=2, ensure_ascii=False)
    target.write("\n")

print(f"{len(models)} models -> {out}")
PY
