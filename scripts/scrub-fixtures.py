#!/usr/bin/env python3
"""Takes the person out of a captured fixture, leaving the shape untouched.

tech.md rule 6: fixtures are captured, never written by hand. A capture comes
off a real machine, so it carries that machine's home directory, the account
name in `ls` output, and whatever a terminal dump happened to have on screen.
None of that is part of any contract, and the repository is public.

So one pass runs over a capture: the home path becomes `/Users/dev`, the
project key Claude Code derives from it becomes `-Users-dev-`, the account
name becomes `dev`, and an address becomes `dev@example.com`. Keys, order,
lengths of the JSON itself and every escape sequence inside a dump survive.
Run it again on the same file and nothing changes.

    scripts/scrub-fixtures.py fixtures/hooks/*.jsonl
    scripts/scrub-fixtures.py --check fixtures/hooks/*.jsonl

`--check` says whether anything is left to scrub and touches nothing, which is
what the capture script and a reviewer both want to ask.
"""

from __future__ import annotations

import os
import re
import sys

# The name of the account a capture was taken on. Read from the environment so
# the next person to capture does not have to edit this file.
USER = os.environ.get("PEEKLE_SCRUB_USER") or os.environ.get("USER") or ""
HOME = os.environ.get("HOME") or ""

STAND_IN = "dev"
ADDRESS = "dev@example.com"


ESCAPE = r"(?:\x1b|\\u001b)\[[0-9;]*[A-Za-z]"
CHUNK = rf"(?:[A-Za-z0-9._%+-]|{ESCAPE})"
PLAIN_ADDRESS = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\Z")


def _address_or_nothing(match: re.Match[str]) -> str:
    """`ADDRESS` when the match reads as one, the match itself when it does not.

    Version strings wear the same shape (`node@22.13.0`), and a fixture is
    full of them. Stripping the escapes and asking for a real domain with a
    lettered end tells the two apart.
    """
    found = match.group(0)
    plain = re.sub(ESCAPE, "", found)
    return ADDRESS if PLAIN_ADDRESS.match(plain) else found


def patterns() -> list[tuple[re.Pattern[str], object]]:
    """What to replace, longest and most specific first."""
    rules: list[tuple[str, object]] = []

    if HOME:
        rules.append((re.escape(HOME), f"/Users/{STAND_IN}"))
    if USER:
        # Claude Code names a project directory after the cwd with every
        # slash and dot folded to a dash, so the user appears in that shape
        # too and has to be replaced in step with the path itself.
        folded = USER.replace(".", "-")
        rules.append((re.escape(f"-Users-{folded}-"), f"-Users-{STAND_IN}-"))
        rules.append((re.escape(f"/Users/{USER}"), f"/Users/{STAND_IN}"))
        # Bare, as `ls -l` prints an owner column.
        rules.append((rf"\b{re.escape(USER)}\b", STAND_IN))

    # An address as a terminal drew it. Claude Code writes cursor moves
    # between the characters, so no part of it is contiguous: the capture
    # holds `name@pr<esc>to<esc>.me`, and in the file the escape itself is
    # the six characters `\u001b`. So the shape is matched loosely and the
    # decision is made on the text with the escapes taken out, which is what
    # a person reading the dump sees.
    rules.append((rf"{CHUNK}+@{CHUNK}+", _address_or_nothing))

    return [(re.compile(pattern), replacement) for pattern, replacement in rules]


def scrub(text: str) -> str:
    for pattern, replacement in patterns():
        text = pattern.sub(replacement, text)
    return text


def main(argv: list[str]) -> int:
    check = "--check" in argv
    paths = [arg for arg in argv if not arg.startswith("--")]
    if not paths:
        print(__doc__, file=sys.stderr)
        return 2

    left = 0
    for path in paths:
        with open(path, encoding="utf-8") as handle:
            before = handle.read()
        after = scrub(before)
        if before == after:
            continue
        left += 1
        if check:
            print(f"{path}: still carries the capturing machine", file=sys.stderr)
            continue
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(after)
        print(f"scrubbed {path}")

    if check and left:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
