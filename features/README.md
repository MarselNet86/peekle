# Features

One entry per shipped feature, newest first: what it does, why, and where to
look at it. The contract for each lives in `tech.md` under the section named in
its row; this log exists so the repository itself remembers what was built and
carries the shots and clips that show it.

Add an entry when a feature merges. Keep it to what a person needs to recognise
the thing on screen — the reasoning belongs in `tech.md`, the mechanics belong
in the code.

| Version | Feature                                                    | Contract           |
| ------- | ---------------------------------------------------------- | ------------------ |
| v60     | [Composer layout](#v60--composer-layout)                   | 6.12, 6.15, 9      |
| v59     | [Permission panel](#v59--permission-panel)                 | 6.3, 6.7, 9        |
| v58     | [Usage badge](#v58--usage-badge)                           | 6.8, 6.10, 6.18, 9 |
| v57     | [Work line](#v57--work-line)                               | 6.12, 9            |
| v56     | [Stop in the field button](#v56--stop-in-the-field-button) | 6.5, 6.15, 9       |

## v60 — Composer layout

2026-09-09 · `de43a62`

![The composer](composer.png)

The field, its settings and the send button are one capsule: the text on top,
the model and the effort under it on the left, the send circle on the right —
the layout Claude Code's own composer uses, for the reason it uses it. Those two
are controls of the message being written, not a line about the session, and as
a separate strip above the field they read as a band between the feed and the
input.

The context ring left that strip for the top right corner, beside `5h` and `7d`.
Three rings answer one question — how much is left — so they stand together. It
is still the button that compacts, and still the only one.

Picker menus grow from the button that opened them rather than appearing whole.

## v59 — Permission panel

2026-09-09 · `ea284d8`

![The permission panel](ask-panel.png)

A permission opens the island a little instead of opening the whole dialogue:
the tool name, the input it was handed, `Deny` dark and `Allow` white. A
hairline under the text leaks through the twenty seconds the panel stands.
Pressing anywhere but the buttons lands in the session, for reading what is
being agreed to. The twenty seconds belong to the panel, never to the hook: the
request stays pending when they run out, and the mark goes on pulsing.

Nothing moves when the island already stands on that session — the person is
reading the thing the request is about.

## v58 — Usage badge

2026-09-08 · `428527f`

[`usage-badge.mp4`](usage-badge.mp4) — twelve seconds, two crossings.

![The badge, frame by frame](usage-badge-frames.png)

Every time the five hour window crosses a ten, the resting island springs wider
and the percent steps out to the left of the ring, stands three and a half
seconds and goes back. Upward only, and never on the first reading. The shape
leads and the number follows it in; on the way out the number leaves first.

Switch under the gear, `[usage] badge` in the config, on by default. Flicking it
on holds a preview until the island rests.

## v57 — Work line

2026-09-08 · `0688104`

![The work line](work-line.png)

The feed prints no tool calls. A run of them folds into one line with a clock:
the mark, the elapsed time and a word while the agent is out, `Worked for 42s`
once it is back. The run is dated from the record before it, because that is
where the person started counting.

## v56 — Stop in the field button

2026-09-08 · `4e41965`

![The stop button](stop-button.png)

One button beside the field: an arrow while there is something to send, a white
square on green while a turn runs, and pressing it ends the turn. The separate
`Stop` is gone. Interrupting always worked — what did not was saying so: no
`Stop` hook arrives for an interrupted turn, so the card sat `Working` until the
sweep. The press now puts the card to rest itself.
