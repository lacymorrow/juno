// usage: swift find-window.swift <pid>
// Prints the CGWindowID of the app's largest layer-0 window wider than 300pt,
// or nothing (exit 1) when there is none. Largest-by-area makes the pick
// deterministic; the prototype's `head -1` depended on list order.
import CoreGraphics
import Foundation

guard CommandLine.arguments.count > 1, let pid = Int(CommandLine.arguments[1]) else { exit(2) }
let list = (CGWindowListCopyWindowInfo([.optionAll], 0) as? [[String: Any]]) ?? []
var best: (id: Int, area: Double)? = nil
for w in list {
    guard (w["kCGWindowOwnerPID"] as? Int) == pid, (w["kCGWindowLayer"] as? Int) == 0,
          let b = w["kCGWindowBounds"] as? [String: Any],
          let width = b["Width"] as? Double, let height = b["Height"] as? Double,
          width > 300, let id = w["kCGWindowNumber"] as? Int else { continue }
    if best == nil || width * height > best!.area { best = (id, width * height) }
}
if let best { print(best.id) } else { exit(1) }
