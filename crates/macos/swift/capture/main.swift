import CoreGraphics
import Foundation
import Metal
import ScreenCaptureKit

// ferrocast-capture: NDJSON requests on stdin, protocol lines on stdout, logs on stderr.
// No text or paths in argv. Exits on stdin EOF.

setvbuf(stdout, nil, _IOLBF, 0)
// Initialise CoreGraphics before any ScreenCaptureKit call: `SCContentFilter(desktopIndependentWindow:)` in a
// process that never touched CoreGraphics aborts with `CGS_REQUIRE_INIT` (seen live: Capture Window saved
// nothing and the helper logged the assertion). Window recording uses the same filter.
_ = CGMainDisplayID()

func sckAvailable() -> Bool {
    if #available(macOS 14, *) { return true }
    return false
}

func handle(_ req: [String: Any]) {
    let id = req["id"]
    guard let cmd = req["cmd"] as? String else {
        fail(id, "invalid", "missing cmd")
        return
    }
    if cmd == "hello" {
        reply(id, ["protocol": protocolVersion, "os": osVersionString(), "version": helperVersion,
                   "sck": sckAvailable(), "metal": MTLCreateSystemDefaultDevice() != nil,
                   "capabilities": sckAvailable() ? ["shareable", "screenshot"] : []])
        return
    }
    guard #available(macOS 14, *) else {
        fail(id, "unsupported-os", "Screen capture needs macOS 14")
        return
    }
    switch cmd {
    case "shareable": handleShareable(id)
    case "screenshot": handleScreenshot(id, req)
    default: fail(id, "invalid", "unknown command: \(cmd)")
    }
}

while let line = readLine(strippingNewline: true) {
    if line.trimmingCharacters(in: .whitespaces).isEmpty { continue }
    guard let data = line.data(using: .utf8),
          let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        fail(nil, "invalid", "not a JSON object")
        continue
    }
    handle(obj)
}
