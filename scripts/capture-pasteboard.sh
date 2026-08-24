#!/usr/bin/env bash
# Captures what macOS writes on the general pasteboard into fixtures/pasteboard/.
#
# tech.md rule 6: fixtures are captured, never written by hand. The screenshot
# discriminator of 6.13 is a claim about someone else's format, and the only
# way to hold it honest is to make the system produce it.
#
# Only the change count and the type names are recorded. Pasteboard contents
# are the user's, and none of them are written to disk.
#
# The clipboard is saved before the run and put back after, including on
# Ctrl-C: probing it must not cost the user what they had copied.
#
# Usage:
#   scripts/capture-pasteboard.sh              every scripted case
#   scripts/capture-pasteboard.sh live <name>  whatever is on the pasteboard now
#
# `live` is how a case nobody scripted gets captured: copy something by hand in
# the app that writes it, then name the case.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/fixtures/pasteboard"
mkdir -p "$OUT"

exec /usr/bin/swift - "$OUT" "${1:-all}" "${2:-live}" <<'SWIFT'
import AppKit

let out = URL(fileURLWithPath: CommandLine.arguments[1])
let mode = CommandLine.arguments[2]
let liveName = CommandLine.arguments[3]
let pb = NSPasteboard.general

/// Every flavor of every item, so the user's clipboard survives the probe.
func save() -> [[NSPasteboard.PasteboardType: Data]] {
    (pb.pasteboardItems ?? []).map { item in
        var flavors: [NSPasteboard.PasteboardType: Data] = [:]
        for type in item.types where item.data(forType: type) != nil {
            flavors[type] = item.data(forType: type)
        }
        return flavors
    }
}

func restore(_ saved: [[NSPasteboard.PasteboardType: Data]]) {
    pb.clearContents()
    let items = saved.map { flavors -> NSPasteboardItem in
        let item = NSPasteboardItem()
        for (type, data) in flavors { item.setData(data, forType: type) }
        return item
    }
    if !items.isEmpty { pb.writeObjects(items) }
}

func shell(_ path: String, _ args: [String]) throws {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: path)
    task.arguments = args
    try task.run()
    task.waitUntilExit()
}

/// Type names only. Contents belong to the user and never reach the fixture.
func write(case name: String, how: String, delta: Int) {
    let types = (pb.types ?? []).map { $0.rawValue }
    let items = (pb.pasteboardItems ?? []).map { $0.types.map { $0.rawValue } }
    let fixture: [String: Any] = [
        "captured_by": "scripts/capture-pasteboard.sh",
        "case": name,
        "how": how,
        "change_count_delta": delta,
        "types": types,
        "items": items,
    ]
    guard let data = try? JSONSerialization.data(
        withJSONObject: fixture, options: [.prettyPrinted, .sortedKeys]) else { return }
    let file = out.appendingPathComponent("\(name).json")
    try? (String(data: data, encoding: .utf8)! + "\n").write(to: file, atomically: true, encoding: .utf8)
    print("\(name): \(items)")
}

let saved = save()
defer { restore(saved) }

if mode == "live" {
    write(case: liveName, how: "copied by hand, captured as found", delta: 0)
    exit(0)
}

// The write Control-Shift-Command-4 makes: screencapture puts the region on
// the pasteboard and nowhere else.
var before = pb.changeCount
try shell("/usr/sbin/screencapture", ["-x", "-c", "-R0,0,24,24"])
write(case: "screenshot", how: "screencapture -x -c -R0,0,24,24", delta: pb.changeCount - before)

// An image copied by an application, through AppKit's own writer.
before = pb.changeCount
let image = NSImage(size: NSSize(width: 24, height: 24))
image.lockFocus()
NSColor.systemGreen.drawSwatch(in: NSRect(x: 0, y: 0, width: 24, height: 24))
image.unlockFocus()
pb.clearContents()
pb.writeObjects([image])
write(case: "image", how: "NSPasteboard.writeObjects with an NSImage", delta: pb.changeCount - before)

// Plain text, the commonest thing on a pasteboard.
before = pb.changeCount
pb.clearContents()
pb.setString("peekle", forType: .string)
write(case: "text", how: "NSPasteboard.setString", delta: pb.changeCount - before)

// A file copied in Finder.
before = pb.changeCount
pb.clearContents()
pb.writeObjects([URL(fileURLWithPath: "/usr/bin/true") as NSURL])
write(case: "file", how: "NSPasteboard.writeObjects with a file URL", delta: pb.changeCount - before)
SWIFT
