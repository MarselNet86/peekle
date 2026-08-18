// Lists what the window server considers on screen right now.
//
// The point is the last column: a window can report isVisible true, carry the
// right collection behavior and the right level, and still not be on the
// active space. Only the window server knows, and only from outside the app.
import CoreGraphics
import Foundation

let wanted = CommandLine.arguments.dropFirst().map { $0.lowercased() }
guard
  let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
    as? [[String: Any]]
else { exit(1) }

var menuBarItems = 0
for w in list {
  let owner = w[kCGWindowOwnerName as String] as? String ?? "?"
  if owner == "Control Center" { menuBarItems += 1 }
  if !wanted.isEmpty && !wanted.contains(where: { owner.lowercased().contains($0) }) { continue }
  let layer = w[kCGWindowLayer as String] as? Int ?? -1
  let b = w[kCGWindowBounds as String] as? [String: Double] ?? [:]
  print("\(owner) layer=\(layer) \(Int(b["Width"] ?? 0))x\(Int(b["Height"] ?? 0))@\(Int(b["X"] ?? 0)),\(Int(b["Y"] ?? 0))")
}

// A full screen space hides the menu bar, so no Control Center items means one
// is active. Cheaper and more reliable than asking the accessibility API.
print("menu_bar_items=\(menuBarItems) full_screen_space=\(menuBarItems == 0)")
