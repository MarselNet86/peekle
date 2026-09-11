# pty

What the CLI drew on its own screen, captured off a real pty. Nothing here is
written by hand: rule 6 applies to a screen exactly as it applies to a hook
payload, and the whole point of these files is that the string Peekle looks for
is the string the CLI actually printed.

Both were taken on Claude Code 2.1.263, 2026-09-11, by spawning
`claude --session-id <id> "<prompt>"` through `peekle_core::pty` in a temporary
folder and keeping everything that came back. Paths are anonymised; not one
other byte is touched, escape sequences included — the mark is matched against
the stream as it arrives, and a fixture with the escapes stripped would be an
easier test than the real thing.

- `trust-question.txt` — a folder that looks like a project (a git repository
  with a `CLAUDE.md`) and has never been trusted: the CLI draws its `Quick
  safety check` question and runs nothing until it is answered. The capture
  carries the answer as well, so what follows a `yes` is in here too.
- `no-question.txt` — the same spawn in a folder that raises no question, which
  is what every screen looks like the rest of the time.

tech.md 6.24.
