import CoreGraphics
import Foundation
import ScreenCaptureKit

private func rect(_ r: CGRect) -> [Double] {
    [Double(r.origin.x), Double(r.origin.y), Double(r.size.width), Double(r.size.height)]
}

/// `shareable`: displays and windows. Exits with code 1 and a `permission-screen` error when
/// Screen Recording is not granted.
@available(macOS 14, *)
func handleShareable(_ id: Any?) {
    if !CGPreflightScreenCaptureAccess() {
        fail(id, "permission-screen", "Screen Recording is not granted")
        exit(1)
    }
    let sem = DispatchSemaphore(value: 0)
    var content: SCShareableContent?
    var failure: Error?
    SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: false) { c, e in
        content = c
        failure = e
        sem.signal()
    }
    if sem.wait(timeout: .now() + 8) == .timedOut {
        fail(id, "internal", "ScreenCaptureKit did not answer")
        return
    }
    guard let content = content else {
        let code = (failure as NSError?)?.code ?? 0
        // SCStreamError.userDeclined = -3801
        if code == -3801 {
            fail(id, "permission-screen", "Screen Recording is not granted")
            exit(1)
        }
        fail(id, "internal", failure?.localizedDescription ?? "no content")
        return
    }
    let mainID = CGMainDisplayID()
    let displays: [[String: Any]] = content.displays.map { d in
        let mode = CGDisplayCopyDisplayMode(d.displayID)
        let scale = mode.map { Double($0.pixelWidth) / Double(max($0.width, 1)) } ?? 1.0
        return ["id": Int(d.displayID), "frame": rect(d.frame), "scale": scale,
                "main": d.displayID == mainID, "name": "Display \(d.displayID)"]
    }
    let windows: [[String: Any]] = content.windows.map { w in
        ["id": Int(w.windowID), "app": w.owningApplication?.applicationName ?? "",
         "bundle": w.owningApplication?.bundleIdentifier ?? "", "title": w.title ?? "",
         "frame": rect(w.frame), "layer": w.windowLayer, "on_screen": w.isOnScreen]
    }
    reply(id, ["displays": displays, "windows": windows])
}
