// usage: swift dump-windows.swift <pid>   (diagnostic: every window the pid owns)
import CoreGraphics
import Foundation
let pid = Int(CommandLine.arguments.dropFirst().first ?? "") ?? -1
for w in (CGWindowListCopyWindowInfo([.optionAll], 0) as? [[String: Any]]) ?? [] {
    guard (w["kCGWindowOwnerPID"] as? Int) == pid else { continue }
    print("id=\(w["kCGWindowNumber"] ?? "?") layer=\(w["kCGWindowLayer"] ?? "?") onscreen=\(w["kCGWindowIsOnscreen"] ?? "no") alpha=\(w["kCGWindowAlpha"] ?? "?") bounds=\(w["kCGWindowBounds"] ?? "?")")
}
