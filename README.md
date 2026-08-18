# Peekle

An overlay on top of Claude Code for macOS.

Start an agent and walk away. When it finishes a turn or asks for permission,
the notch of the MacBook grows, shows what the agent said, takes your answer,
and collapses again. The answer goes straight back into the turn, so you never
focus the terminal.

There is one surface and it is the island. No second window, no HUD, no
separate prompt panel.

Contracts, types and the roadmap live in `tech.md`. It is the source of truth;
this file only explains how to run what exists today.

## Status

The stage 1 framework is in. Feature slices are in progress.

| Surface                | State                                                    |
| ---------------------- | -------------------------------------------------------- |
| Island toast           | Working end to end, hook to pixel                        |
| Island shape           | S1                                                       |
| Feed, Stop, permission | Not started, S2 to S4. A blocking hook waits out its     |
|                        | timeout until then, so do not point a live session at it |
| Hook server            | All endpoints of tech.md 6.2 answer                      |
| Usage bars             | Fake provider only, the account provider lands with S7   |
| Hotkey, CLI, packaging | Not started, S8 to S10                                   |

## Requirements

macOS 13+, Node 22.13+, pnpm, Rust from `rust-toolchain.toml`.

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
./scripts/demo.sh island    # a toast, hides itself
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

## Config

`~/Library/Application Support/peekle/config.toml`, mode 0600. Written by
`peekle init`, read at startup. Every key and default is in tech.md 6.8.

## Layout

```
src/lib/ui/          primitives, nothing else draws
src/lib/logic/       pure TypeScript, property tested
src/lib/bridge/      the only module that touches @tauri-apps/api
src-tauri/src/       windows, panels, commands, state
crates/peekle-core   types, config, pending registry, label classifier
crates/peekle-server axum router for the hook endpoints
fixtures/hooks/      captured payloads, never hand written
```

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
```

The script backs up `~/.claude/settings.json`, installs capture handlers, and
restores the file on exit including on Ctrl-C. Drive a Claude Code session in
another terminal while it runs.
