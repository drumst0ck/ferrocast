// Lists windows: id, owner, layer, sharing state, onscreen, bounds. Optional owner filter (substring, case-insensitive).
import CoreGraphics
import Foundation

let filter = CommandLine.arguments.dropFirst().first?.lowercased()
let list = (CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]]) ?? []
for w in list {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    if let f = filter, !owner.lowercased().contains(f) { continue }
    let b = w[kCGWindowBounds as String] as? [String: Double] ?? [:]
    let on = (w[kCGWindowIsOnscreen as String] as? Bool) ?? false
    print("id=\(w[kCGWindowNumber as String] ?? "?") owner=\(owner) layer=\(w[kCGWindowLayer as String] ?? "?") sharing=\(w[kCGWindowSharingState as String] ?? "?") onscreen=\(on) bounds=\(Int(b["X"] ?? 0)),\(Int(b["Y"] ?? 0)),\(Int(b["Width"] ?? 0)),\(Int(b["Height"] ?? 0))")
}
