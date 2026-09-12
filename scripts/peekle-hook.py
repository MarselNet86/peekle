#!/usr/bin/env python3
"""Peekle's hook handler. Installed by `peekle init` as ~/.claude/peekle/peekle-hook.py

Two jobs, and nothing else:

1. Add `pid` and `tty` to the payload. This script runs as a child of the
   Claude Code process, which makes it the only place those two facts exist.
   An http handler arrives over a socket and knows nothing about the process
   that triggered it. Without them there is no way to find the tmux pane a
   session lives in. tech.md 6.1.

2. Post the payload to the local Peekle server and, for the two hooks that can
   block, hand its answer back to Claude Code on stdout.

It decides nothing itself. If Peekle is down, slow, or switched off, this exits
0 with no output, which means "no decision" and Claude Code carries on exactly
as it would without Peekle installed. For a permission request that is the
whole safety story: the question goes back to the terminal rather than being
swallowed. tech.md 6.1.

Never logs the payload or the token: it runs on every event of every agent, and
its stderr lands in the user's terminal. Rule 11.
"""

import json
import os
import subprocess
import sys
import urllib.error
import urllib.request

def config_file():
    """Where `peekle init` wrote the config, under the name the platform uses.

    The same three places `directories::ProjectDirs` hands the Rust side, and
    they have to agree exactly: this file carries the port and the token, and a
    handler that cannot find it posts nowhere and exits quiet -- which is what
    every hook on Windows did until v82.7, so the island showed no feed, no
    spinner, and a reply nothing ever confirmed. tech.md 6.8 and 6.27.
    """
    if sys.platform == "darwin":
        return os.path.expanduser("~/Library/Application Support/peekle/config.toml")
    if os.name == "nt":
        roaming = os.environ.get("APPDATA") or os.path.expanduser("~/AppData/Roaming")
        return os.path.join(roaming, "peekle", "config", "config.toml")
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    return os.path.join(base, "peekle", "config.toml")


CONFIG = config_file()

# Only these two ever wait for a person. Everything else is fire and forget, and
# a feed event that blocks the agent would be a bug with a stopwatch on it.
BLOCKING = {"Stop", "PermissionRequest"}

# Ceilings, not working windows: Peekle answers on its own schedule well inside
# these. They exist so a hung server cannot park an agent forever. tech.md 6.8.
BLOCKING_TIMEOUT = 3600
EVENT_TIMEOUT = 5


def config():
    """port and token out of config.toml.

    Hand-rolled rather than tomllib: this has to run under whatever python3 the
    user has, and tomllib only exists from 3.11. The file is written by
    `peekle init` and its shape is known. tech.md 6.8.
    """
    port, token, section = 47821, None, None
    try:
        with open(CONFIG, encoding="utf-8") as handle:
            for line in handle:
                line = line.split("#", 1)[0].strip()
                if line.startswith("[") and line.endswith("]"):
                    section = line[1:-1]
                    continue
                if section != "server" or "=" not in line:
                    continue
                key, _, value = line.partition("=")
                key, value = key.strip(), value.strip().strip('"')
                if key == "port":
                    port = int(value)
                elif key == "token":
                    token = value
    except (OSError, ValueError):
        return None, None
    return port, token


def tty_of(pid):
    """The terminal the agent is attached to, or None.

    None is ordinary rather than exceptional: an agent hosted by an IDE
    extension talks over pipes and has no controlling terminal at all. That
    session simply takes the turn-boundary path. tech.md 6.5.
    """
    try:
        out = subprocess.run(
            ["/bin/ps", "-p", str(pid), "-o", "tty="],
            capture_output=True,
            text=True,
            timeout=2,
        )
    except (OSError, subprocess.SubprocessError):
        return None

    name = out.stdout.strip()
    if not name or name in ("??", "-"):
        return None
    return name if name.startswith("/dev/") else "/dev/" + name


def endpoint(event):
    """tech.md 6.1, the endpoint column."""
    if event == "Stop":
        return "stop"
    if event == "PermissionRequest":
        return "permission"
    if event == "Notification":
        return "notification"
    if event in ("UserPromptSubmit", "PreToolUse", "PostToolUse"):
        return "feed"
    return "session"


def main():
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        # Not our payload to interpret. Say nothing and let the turn continue.
        return 0

    event = payload.get("hook_event_name", "")
    port, token = config()
    if not token:
        return 0

    # The parent is the agent: Claude Code runs this script directly.
    payload["pid"] = os.getppid()
    tty = tty_of(payload["pid"])
    if tty:
        payload["tty"] = tty

    url = f"http://127.0.0.1:{port}/v1/h/{token}/{endpoint(event)}"
    blocking = event in BLOCKING
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"content-type": "application/json"},
        method="POST",
    )

    try:
        timeout = BLOCKING_TIMEOUT if blocking else EVENT_TIMEOUT
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read()
    except (urllib.error.URLError, OSError, ValueError):
        # Peekle is not running, or refused. No decision: Claude Code behaves
        # as if Peekle were not installed, and a permission prompt appears in
        # the terminal where the user can answer it. tech.md 6.1.
        return 0

    if not blocking:
        return 0

    try:
        decision = json.loads(body)
    except (json.JSONDecodeError, ValueError):
        return 0

    # An empty object is a real answer meaning "no decision". Printing it would
    # be harmless, but printing nothing is what Claude Code documents.
    if isinstance(decision, dict) and decision:
        sys.stdout.write(json.dumps(decision))
    return 0


if __name__ == "__main__":
    sys.exit(main())
