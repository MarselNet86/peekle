# Contributing

Pull requests are welcome. Open an issue first if you plan something big, so
we do not both build it.

Contracts, types and the roadmap live in `tech.md`, which is kept out of the
repository by the owner's decision; the code carries the section numbers it
implements, and the feature log in [`docs/features/README.md`](../docs/features/README.md)
records what shipped, with a shot of each.

## Requirements

macOS 13+, Node 22.13+, pnpm and Rust from `rust-toolchain.toml`.

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

## Drive the island without an agent

`scripts/demo.sh` plays the part of Claude Code:

```sh
./scripts/demo.sh island    # a toast: the notch grows, then collapses
./scripts/demo.sh tasks     # three tasks into the registry
./scripts/demo.sh clear     # empty the task list
./scripts/demo.sh stop      # blocks like a real Stop hook
./scripts/demo.sh health    # is the server up
```

## How it fits together

Claude Code drives Peekle through hooks. Peekle runs an HTTP server on
loopback; the hooks are `type: "command"` entries that run
`~/.claude/peekle/peekle-hook.py`, which posts the payload to that server and,
for the two hooks that can block, writes the answer back on stdout. The hook
request is the event, the HTTP response body is the decision Claude Code
executes. If the server is not there the script exits 0 with no output, which
Claude Code reads as "no decision".

```
Claude Code turn
  |  peekle-hook.py: POST /v1/h/<token>/<endpoint>
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

## The CLI

```sh
cargo build -p peekle-cli --bin peekle
./target/debug/peekle init        # wire the hooks into ~/.claude/settings.json
./target/debug/peekle doctor      # what is wrong and what to do about it
./target/debug/peekle status      # the same as JSON
./target/debug/peekle uninstall   # take back only its own entries
```

`init` merges its handlers into `~/.claude/settings.json`, never removes anyone
else's, takes a timestamped backup first, and changes nothing on a second run.

Users get the same binary inside the app: `pnpm tauri build` runs
`scripts/build-cli.mjs` before the Rust build, which compiles the CLI for the
build target (both slices, through lipo, for a universal build) and puts it at
`target/cli/peekle`; `bundle.macOS.files` copies it to
`Peekle.app/Contents/MacOS/peekle`, and the Homebrew cask links it into
`bin`. `peekle --version` prints the workspace version.

## Config

Written by `peekle init`, read at startup:
`~/Library/Application Support/peekle/config.toml`, mode 0600 because it holds
the hook token. Every key and default is in tech.md 6.8.

## Layout

```
src/lib/ui/              primitives, nothing else draws
src/lib/logic/           pure TypeScript, property tested
src/lib/bridge/          the only module that touches @tauri-apps/api
src-tauri/src/           windows, panels, commands, state
src-tauri/src/platform/  every line that knows which OS this is
crates/peekle-core       types, config, pending registry, label classifier
crates/peekle-server     axum router for the hook endpoints
crates/peekle-update     the GitHub release check
crates/peekle-cli        peekle init, doctor, status, uninstall
tests/fixtures/          captured payloads and shapes, never hand written
docs/features/           the feature log and a shot of every visible change
.github/homebrew/        the cask template release.yml renders
```

## Conventions

- Every visual element comes from `src/lib/ui`; nothing else draws.
- Every visible change ships with a shot in `docs/features/shots/` and an
  entry in `docs/features/README.md`.
- Every blocking hook request resolves exactly once on every path: answer,
  cancel, timeout, bypass, error. A leaked pending request hangs a live agent.
- No `unwrap` outside tests. Never in a hook handler.
- Secrets never reach a log: the server token, the OAuth token and Keychain
  contents are logged by length, never by value.
- The Keychain dialog is raised only by a user action, never at startup or from
  a background poll.
- Commits follow Conventional Commits, in English, in the imperative.

## Tests

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace     # unit, contract, golden payload, property
pnpm lint && pnpm check
pnpm test                  # component and property tests
pnpm test:e2e              # the routes under vite dev, Chromium and WebKit
shellcheck install.sh
pnpm tauri build --debug
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

## Screenshots into a session

Take one with ⌃⇧⌘4, which puts it on the clipboard. The notch offers to attach
it for five seconds; press ⌘1 and it lands in the field of the
session you were last working in.

Only a session Peekle started can take one: there is no way to type into a
process Peekle did not start. Detection reads the type names on the clipboard
and never the contents, so nothing asks for permission until you press the
key. The image is written to `~/Library/Caches/peekle/shots/` and the path
travels to the agent as a line of the message.

## Releasing

Bump the version in `Cargo.toml`, `package.json` and `src-tauri/tauri.conf.json`
in one commit, then tag it `vX.Y.Z` and push the tag. The tag has to match
the version in `tauri.conf.json`; the workflow checks that first and stops if
it does not. It then builds the universal dmg with the CLI inside, signs it ad
hoc, renders the Homebrew cask from `.github/homebrew/peekle.rb` with the
version and the dmg's checksum, and attaches both `Peekle-mac-universal.dmg`
and `peekle.rb` to the GitHub release. The tap,
[MarselNet86/homebrew-tap](https://github.com/MarselNet86/homebrew-tap),
copies that `peekle.rb` on its next hourly run, or at once when its `bump`
workflow is run by hand. Before attaching anything the job installs the dmg
it just built through `install.sh` and checks that the command inside
answers with the tag's version. Installed copies pick the release up on
their next check.

`install.sh` is the path for people without Homebrew: it downloads the
latest dmg, checks it against the sha256 in that release's `peekle.rb`,
copies the app, clears quarantine, links the command and runs `init`.
`PEEKLE_DMG` installs a local dmg instead, `PEEKLE_APPDIR` changes where
the app goes, and `PEEKLE_INSTALL_ONLY=1` stops after the copy, which is
how CI runs it.
