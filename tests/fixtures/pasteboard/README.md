# Captured pasteboard shapes

Every file here comes from `scripts/capture-pasteboard.sh`, which makes macOS
write the pasteboard itself and records what turned up. None of it is written
by hand.

The screenshot discriminator of tech.md 6.13 is a claim about someone else's
format, and it decides whether the island offers to attach at all. Guessing at
it would produce a test that passes while the product misreads every copy the
user makes.

Type names only. Pasteboard contents belong to the user and never reach a file
here.

## Captured

- `screenshot.json` — `screencapture -x -c`, the write Control-Shift-Command-4
  makes: one item carrying exactly `public.png`
- `image.json` — an image written by AppKit: one item carrying `public.tiff`
- `text.json` — plain text: `public.utf8-plain-text`
- `text-from-browser.json` — text copied out of Chrome by hand, carrying
  `public.html` and two Chromium flavors alongside the plain text
- `file.json` — a file URL, the shape a Finder copy takes

The declared types of the pasteboard are wider than the types of its item:
NSPasteboard synthesises `public.tiff` from a PNG, so the screenshot pasteboard
declares TIFF while its item does not. The discriminator reads the item.

## When a capture disagrees with tech.md

The capture wins. Fix section 6, bump the core version, and land the contract
change before the code that depends on it. tech.md section 13.
