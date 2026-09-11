# Peekle

An overlay on top of Claude Code. macOS is the home platform; Windows and
Linux run the same island through a platform layer (tech.md 6.27).

Start an agent and walk away. When it finishes a turn or asks for permission,
the notch of the MacBook grows, shows what the agent said, takes your answer,
and collapses again. The answer goes straight back into the turn, so you never
focus the terminal.

There is one surface and it is the island. No second window, no HUD, no
separate prompt panel.

Contracts, types and the roadmap live in `tech.md`. It is the source of truth;
this file only explains how to run what exists today.

## Installation

| Platform          | Download                                                                                                                                                                                                                                      |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS (universal) | [Peekle-mac-universal.dmg](https://github.com/MarselNet86/peekle/releases/latest/download/Peekle-mac-universal.dmg)                                                                                                                           |
| Windows           | [Peekle-win-x64-setup.exe](https://github.com/MarselNet86/peekle/releases/latest/download/Peekle-win-x64-setup.exe)                                                                                                                           |
| Linux             | [Peekle-linux-x86_64.AppImage](https://github.com/MarselNet86/peekle/releases/latest/download/Peekle-linux-x86_64.AppImage) · [Peekle-linux-amd64.deb](https://github.com/MarselNet86/peekle/releases/latest/download/Peekle-linux-amd64.deb) |

Every link points at the newest release, and the names carry no version, so a
link written down once keeps working. Each build comes off a GitHub runner of
that platform. `Peekle-win-x64.msi`, for a managed rollout, sits beside the
setup on the release page.

Nothing is code-signed by a paid certificate, on any of the three, so each
platform asks once:

- **macOS**: the bundle carries an ad-hoc signature. Open it the first time
  with right-click → Open. macOS 13 or newer.
- **Windows**: SmartScreen says unknown publisher; More info → Run anyway. The
  installer pulls WebView2 if the machine has none.
- **Linux**: `chmod +x Peekle-linux-x86_64.AppImage` and run it, or
  `sudo apt install ./Peekle-linux-amd64.deb`. Needs webkit2gtk 4.1, so
  Ubuntu 22.04 or newer and equivalents.

What differs away from macOS is listed in tech.md 6.27, and the short of it:
the island lives on the primary monitor only, takes screenshots from any of
them, and has no notch to grow out of, so at rest it is a floating pill. The
native behaviour on Windows and Linux has not been walked by hand yet — the
builds compile and bundle on CI, and the first live run on each is the manual
checklist of tech.md 15.

## Status

Stage 1 is in and most of stage 2 with it.

| Surface             | State                                                          |
| ------------------- | -------------------------------------------------------------- |
| Island              | Working, and it reaches another app's full screen space        |
| Feed and sessions   | Working, fed by the hooks                                      |
| Stop and permission | Working, answers reach the agent without touching the terminal |
| Hook server         | Every endpoint of tech.md 6.2 answers                          |
| Hotkey              | Working, ⌥⇧Q toggles the bypass                                |
| CLI                 | init, uninstall, doctor, status. `off` is not built            |
| Usage bars          | Working from the account, dashes with a reason when it cannot  |
| Screenshots         | ⌃⇧⌘4 offers to attach the shot to a session Peekle started     |
| Packaging           | Installers for all three platforms, built by CI on a tag       |
| First run           | Not started, S11                                               |

## Requirements

To build it: macOS 13+, Windows 10+ or a Linux with webkit2gtk 4.1, plus
Node 22.13+, pnpm and Rust from `rust-toolchain.toml`.

```sh
pnpm install
```

## Run it

```sh
pnpm tauri dev
```

Build with the Tauri CLI, not with `cargo build`. A plain cargo build produces
a dev binary that points its windows at the vite dev server, so with no server
running the windows load nothing and stay invisible.

Nothing appears at startup on purpose. Peekle has no dock icon, no tray, no
menu bar item and no close button. The island exists only while it has
something to say.

## Drive the island

`peekle init` (S9) is what wires a real Claude Code session. Until then,
`scripts/demo.sh` plays the part of the agent:

```sh
./scripts/demo.sh island    # a toast: the notch grows, then collapses
./scripts/demo.sh tasks     # three tasks into the registry
./scripts/demo.sh clear     # empty the task list
./scripts/demo.sh stop      # blocks like a real Stop hook
./scripts/demo.sh health    # is the server up
```

`stop` blocks the way a real hook does. Nothing answers it before S3, so it
waits out `behavior.prompt_timeout_secs` and the turn then ends normally.

## How it fits together

Claude Code drives Peekle through hooks. Peekle runs an HTTP server on
loopback, the hooks are `type: "http"` and post to it. The hook request is the
event, the HTTP response body is the decision Claude Code executes.

```
Claude Code turn
  |  POST /v1/h/<token>/<endpoint>
  v
peekle-server (axum, 127.0.0.1)
  |  registers a pending request, emits a Tauri event
  v
state in Rust  ->  island
  |                    |
  |  <- answer_prompt() <-
  v
HTTP response body  ->  the turn continues
```

If Peekle is not running the connection fails, Claude Code treats that as a
non-blocking error and works normally. The degradation is free: an agent never
hangs on a dead overlay.

## Install it into Claude Code

```sh
cargo build -p peekle-cli --bin peekle
./target/debug/peekle init
```

`init` merges its handlers into `~/.claude/settings.json`, never removes anyone
else's, takes a timestamped backup first, and changes nothing on a second run.
`uninstall` takes back only its own entries.

```sh
./target/debug/peekle doctor    # what is wrong and what to do about it
./target/debug/peekle status    # the same as JSON
```

`peekle off` is not built. Toggling from a shell means writing the config and
having the running app notice, and the config watcher that tech.md 6.8 promises
has not been built by any slice yet. Until then the toggle is the hotkey.

## Config

Written by `peekle init`, read at startup. Every key and default is in
tech.md 6.8.

```
macOS    ~/Library/Application Support/peekle/config.toml   mode 0600
Linux    ~/.config/peekle/config.toml                       mode 0600
Windows  %APPDATA%\peekle\config\config.toml
```

The file holds the hook token, so on macOS and Linux it is written 0600.
Windows has no such mode; the file sits in the user's profile, whose ACL
already admits nobody else. tech.md 6.27.

## Layout

```
src/lib/ui/          primitives, nothing else draws
src/lib/logic/       pure TypeScript, property tested
src/lib/bridge/      the only module that touches @tauri-apps/api
src-tauri/src/       windows, panels, commands, state
src-tauri/src/platform/  every line that knows which OS this is
crates/peekle-core   types, config, pending registry, label classifier
crates/peekle-server axum router for the hook endpoints
fixtures/hooks/      captured payloads, never hand written
fixtures/pasteboard/ captured pasteboard shapes, never hand written
```

## Screenshots

Take one with ⌃⇧⌘4, which puts it on the clipboard. The notch offers to attach
it for five seconds; press the up arrow and it lands in the field of the
session you were last working in, where you say what you want done with it.

Only a session Peekle started can take one. There is no way to type into a
process Peekle did not start (tech.md 6.5), so a screenshot has nowhere to go
in a session you began in your own terminal. Start one with New session in the
island.

Detection reads the type names on the clipboard and never the contents, so
nothing asks for permission until you press the key. The image is written to
`~/Library/Caches/peekle/shots/` and the path travels to the agent as a line of
the message.

## Tests

```sh
cargo test --workspace   # unit, contract, golden payload, property
pnpm test                # component and property tests
pnpm test:e2e            # the routes under vite dev
```

Native panel behaviour is not covered by any of these. It lives in the manual
checklist of tech.md section 15 and has to be walked by hand before a release.

## Capturing fixtures

Hook payloads are captured off a live session, never written by hand. A payload
invented from documentation that drifts from what Claude Code sends is worse
than no test at all.

```sh
./scripts/capture-hooks.sh 180
./scripts/capture-hooks.sh --restore   # after a hard kill
```

The same rule holds for the clipboard shapes behind the screenshot offer. The
script makes macOS write each case and records the type names only; it saves
and restores whatever you had copied.

```sh
./scripts/capture-pasteboard.sh              # every scripted case
./scripts/capture-pasteboard.sh live <name>  # whatever is on the clipboard now
```

The script backs up `~/.claude/settings.json`, installs capture handlers, and
restores the file on exit including on Ctrl-C. Drive a Claude Code session in
another terminal while it runs.

The trap cannot cover SIGKILL, and settings left pointing at a dead capture
server is a bad way to find that out, so the backup path is written to a marker
file that `--restore` reads.
