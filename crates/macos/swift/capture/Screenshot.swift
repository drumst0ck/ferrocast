import CoreGraphics
import Foundation
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

private func number(_ v: Any?) -> Double? { (v as? NSNumber)?.doubleValue }

private func permissionDenied(_ id: Any?) -> Never {
    fail(id, "permission-screen", "Screen Recording is not granted")
    exit(1)
}

/// Encode `image` as PNG to `<out>.partial`, then rename over `out`, so a failure never leaves a
/// half-written file at `out`.
private func writePNG(_ image: CGImage, to out: String) -> Bool {
    let tmp = out + ".partial"
    let fm = FileManager.default
    try? fm.createDirectory(atPath: (out as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
    try? fm.removeItem(atPath: tmp)
    guard let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: tmp) as CFURL, UTType.png.identifier as CFString, 1, nil) else {
        return false
    }
    CGImageDestinationAddImage(dest, image, nil)
    guard CGImageDestinationFinalize(dest) else {
        try? fm.removeItem(atPath: tmp)
        return false
    }
    do {
        if fm.fileExists(atPath: out) { try fm.removeItem(atPath: out) }
        try fm.moveItem(atPath: tmp, toPath: out)
        return true
    } catch {
        try? fm.removeItem(atPath: tmp)
        return false
    }
}

/// `screenshot`: display / window / rect to a PNG at the native pixel size. Exits 1 with a
/// `permission-screen` error when Screen Recording is not granted.
@available(macOS 14, *)
func handleScreenshot(_ id: Any?, _ req: [String: Any]) {
    guard let out = req["out"] as? String, out.hasPrefix("/"),
          let target = req["target"] as? [String: Any], let kind = target["kind"] as? String else {
        fail(id, "invalid", "screenshot needs target and an absolute out path")
        return
    }
    if !CGPreflightScreenCaptureAccess() { permissionDenied(id) }

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
        if ((failure as NSError?)?.code ?? 0) == -3801 { permissionDenied(id) }
        fail(id, "internal", failure?.localizedDescription ?? "no content")
        return
    }

    let config = SCStreamConfiguration()
    config.showsCursor = req["cursor"] as? Bool ?? false
    config.ignoreShadowsSingleWindow = !(req["window_shadow"] as? Bool ?? false)
    let filter: SCContentFilter
    var points = CGSize.zero  // size of the captured area in points
    switch kind {
    case "display", "rect":
        guard let did = number(target["display"]),
              let display = content.displays.first(where: { Double($0.displayID) == did }) else {
            fail(id, "not-found", "display \(number(target["display"]) ?? -1) not found")
            return
        }
        filter = SCContentFilter(display: display, excludingWindows: [])
        points = CGSize(width: display.frame.width, height: display.frame.height)
        if kind == "rect" {
            guard let r = target["rect"] as? [Any], r.count == 4, let x = number(r[0]), let y = number(r[1]),
                  let w = number(r[2]), let h = number(r[3]), w >= 1, h >= 1 else {
                fail(id, "invalid", "rect must be [x, y, w, h] with positive size")
                return
            }
            // Clamp to the display so the output size matches what is captured.
            let clamped = CGRect(x: x, y: y, width: w, height: h).intersection(CGRect(origin: .zero, size: points))
            if clamped.isNull || clamped.width < 1 || clamped.height < 1 {
                fail(id, "invalid", "rect is outside the display")
                return
            }
            config.sourceRect = clamped
            points = clamped.size
        }
    case "window":
        guard let wid = number(target["window"]),
              let window = content.windows.first(where: { Double($0.windowID) == wid }) else {
            fail(id, "not-found", "window \(number(target["window"]) ?? -1) not found")
            return
        }
        filter = SCContentFilter(desktopIndependentWindow: window)
        points = window.frame.size
    default:
        fail(id, "invalid", "unknown target kind: \(kind)")
        return
    }
    let scale = Double(filter.pointPixelScale)
    config.width = max(1, Int((Double(points.width) * scale).rounded()))
    config.height = max(1, Int((Double(points.height) * scale).rounded()))

    var image: CGImage?
    var shotError: Error?
    SCScreenshotManager.captureImage(contentFilter: filter, configuration: config) { i, e in
        image = i
        shotError = e
        sem.signal()
    }
    if sem.wait(timeout: .now() + 15) == .timedOut {
        fail(id, "internal", "screenshot timed out")
        return
    }
    guard let image = image else {
        if ((shotError as NSError?)?.code ?? 0) == -3801 { permissionDenied(id) }
        fail(id, "internal", shotError?.localizedDescription ?? "no image")
        return
    }
    guard writePNG(image, to: out) else {
        fail(id, "internal", "could not write \(out)")
        return
    }
    reply(id, ["path": out, "size": [image.width, image.height], "scale": scale])
}
