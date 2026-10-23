import AVFoundation
import CoreGraphics
import CoreImage
import Foundation

// Camera overlay (P4, S17): the webcam take drawn over the styled frame by `render.frame` and by the
// export. Only drawn when project.json has `camera` with `visible: true` AND `sources.camera` points
// at an existing file; a project without them takes none of this code (S19: same bytes as before).
//
// Time: camera file time = screen source time - `sources.camera.offset`. Before the file starts
// (negative) or at/after its end nothing is drawn, never a frozen frame.
// Placement: `rect` below is `capture::camera::rect` ported EXACTLY (sizes are fractions of the OUTPUT
// width, whole pixels, clamped inside the canvas); `cornerRadius` is `camera::corner_radius`.
// The overlay is composed AFTER the screen picture (and its zoom crop), so it stays fixed in the output.
// Decoding: the generator for random access and PNG frames, a second sequential reader (`cameraState`,
// Decode.swift) for consecutive raw/export frames: one reader per track.

/// Layout and source of the camera overlay, read from project.json.
struct CameraSpec: Equatable {
    var shape = "circle"
    var corner = "bottom_right"
    var size = 0.20
    var margin = 0.02
    var mirror = true
    let url: URL
    let offset: Double

    /// nil unless the camera is visible and its file exists.
    init?(json: [String: Any], project: String) {
        guard let c = json["camera"] as? [String: Any], (c["visible"] as? Bool) ?? true,
              let src = (json["sources"] as? [String: Any])?["camera"] as? [String: Any],
              let file = src["file"] as? String, !file.isEmpty, !file.contains("/"), !file.contains("..") else { return nil }
        let path = (project as NSString).appendingPathComponent(file)
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        url = URL(fileURLWithPath: path)
        offset = num(src["offset"]).flatMap { $0.isFinite ? $0 : nil } ?? 0
        if let v = c["shape"] as? String, ["circle", "rounded", "square"].contains(v) { shape = v }
        if let v = c["corner"] as? String, ["top_left", "top_right", "bottom_left", "bottom_right"].contains(v) { corner = v }
        if let v = num(c["size"]), v.isFinite { size = min(max(v, 0.10), 0.40) }
        if let v = num(c["margin"]), v.isFinite { margin = min(max(v, 0), 0.25) }
        if let v = c["mirror"] as? Bool { mirror = v }
    }

    /// Part of the preview frame cache key (the layout is also covered by project.json's mtime).
    var cacheKey: String { "\(url.path)|\(mtime(url.path))" }
}

/// `camera::rect`: top-left pixel rectangle of the (square) overlay.
func cameraRect(canvasW: Double, canvasH: Double, spec: CameraSpec) -> CGRect {
    let side = min((spec.size * canvasW).rounded(), min(canvasW, canvasH)).clamped(atLeast: 1)
    let margin = (spec.margin * canvasW).rounded().clamped(atLeast: 0)
    let mx = min(margin, (canvasW - side).clamped(atLeast: 0)), my = min(margin, (canvasH - side).clamped(atLeast: 0))
    let left = spec.corner == "top_left" || spec.corner == "bottom_left"
    let top = spec.corner == "top_left" || spec.corner == "top_right"
    return CGRect(x: left ? mx : canvasW - side - mx, y: top ? my : canvasH - side - my, width: side, height: side)
}

/// `camera::corner_radius` (`ROUNDED_RADIUS` = 0.2).
func cameraCornerRadius(shape: String, side: Double) -> Double {
    switch shape {
    case "circle": return side / 2
    case "rounded": return side * 0.2
    default: return 0
    }
}

private extension Double {
    func clamped(atLeast lo: Double) -> Double { Swift.max(self, lo) }
}

/// A camera frame ready to be drawn, with its layout.
struct CameraDraw {
    let frame: TakeFrame
    let spec: CameraSpec
}

/// The camera frame for screen source time `t`, or nil when the file has not started / has ended (or
/// cannot be read: logged, the frame is rendered without the overlay).
func cameraDraw(_ spec: CameraSpec, at t: Double, sequential: Bool) -> CameraDraw? {
    let ct = t - spec.offset
    do {
        let r = try reader(for: spec.url)
        guard ct >= 0, ct < r.duration else { return nil }
        if sequential {
            return CameraDraw(frame: try takeFrameAuto(at: spec.url, t: ct, mode: .auto, state: cameraState).frame, spec: spec)
        }
        return CameraDraw(frame: TakeFrame(cg: try takeFrame(at: spec.url, t: ct)), spec: spec)
    } catch {
        log("camera overlay skipped at \(t) s: \(error)")
        return nil
    }
}

/// Mask, placement and shadow of the overlay depend only on the style and the geometry: kept between frames.
private struct CameraFxKey: Equatable {
    let canvas: CGSize
    let rect: CGRect
    let radius: CGFloat
    let sigma: CGFloat
    let dy: CGFloat
    let opacity: Double
}

private struct CameraFx {
    let key: CameraFxKey
    /// Coverage mask of the overlay shape on the canvas, nil for a square shape.
    let mask: CIImage?
    let shadow: CIImage?
}

private var cameraFxCache: CameraFx?

/// Draw the overlay over `base` (the finished styled frame). The shadow follows the card's: same
/// opacity, half its blur and offset (the overlay is much smaller than the card).
func compositeCamera(_ draw: CameraDraw, over base: CIImage, style: RenderStyle, geo: RenderGeometry, canvas: CGRect) -> CIImage {
    let spec = draw.spec
    let r = cameraRect(canvasW: Double(geo.canvas.width), canvasH: Double(geo.canvas.height), spec: spec)
    let side = r.width
    let rectCI = CGRect(x: r.minX, y: geo.canvas.height - r.maxY, width: side, height: side)
    let radius = CGFloat(cameraCornerRadius(shape: spec.shape, side: Double(side)))
    let opacity = min(max(style.shadowOpacity, 0), 1)
    let key = CameraFxKey(canvas: geo.canvas, rect: rectCI, radius: radius, sigma: geo.shadowSigma / 2, dy: geo.shadowDy / 2, opacity: opacity)
    let fx: CameraFx
    if let hit = cameraFxCache, hit.key == key { fx = hit } else {
        var mask: CIImage?
        var shape: CIImage?
        // canvas-sized like the card's mask: the blur below clamps its input to the extent, so a mask
        // as small as the overlay would smear its edge pixels over the whole canvas
        if let cg = maskImage(canvas: geo.canvas, rect: r, radius: radius) {
            let full = rawImage(cg)
            if radius > 0 { mask = full }
            shape = full.applyingFilter("CIMaskToAlpha")
        }
        var shadow: CIImage?
        if let shape, opacity > 0, key.sigma > 0.01 || key.dy != 0 {
            shadow = blurred(shape, sigma: key.sigma, within: canvas.insetBy(dx: -4 * key.sigma, dy: -4 * key.sigma))
                .applyingFilter("CIColorMatrix", parameters: [
                    "inputRVector": CIVector(x: 0, y: 0, z: 0, w: 0), "inputGVector": CIVector(x: 0, y: 0, z: 0, w: 0),
                    "inputBVector": CIVector(x: 0, y: 0, z: 0, w: 0), "inputAVector": CIVector(x: 0, y: 0, z: 0, w: CGFloat(opacity)),
                ])
                .transformed(by: CGAffineTransform(translationX: 0, y: -key.dy))
        }
        fx = CameraFx(key: key, mask: mask, shadow: shadow)
        cameraFxCache = fx
    }

    // aspect-fill the camera frame into the square (centre crop)
    let img = draw.frame.image
    let e = img.extent
    let s = max(side / e.width, side / e.height)
    let size = CGSize(width: (e.width * s).rounded(.up), height: (e.height * s).rounded(.up))
    let origin = CGPoint(x: rectCI.minX - ((size.width - side) / 2).rounded(), y: rectCI.minY - ((size.height - side) / 2).rounded())
    var picture = scaled(img, to: size, at: origin).cropped(to: rectCI)
    if spec.mirror {
        picture = picture.transformed(by: CGAffineTransform(scaleX: -1, y: 1).concatenating(CGAffineTransform(translationX: 2 * rectCI.minX + side, y: 0)))
    }
    if let mask = fx.mask {
        picture = picture.applyingFilter("CIBlendWithMask", parameters: [
            kCIInputBackgroundImageKey: CIImage(color: .clear).cropped(to: rectCI), kCIInputMaskImageKey: mask,
        ])
    }
    var result = base
    if let shadow = fx.shadow { result = shadow.composited(over: result) }
    return picture.composited(over: result).cropped(to: canvas)
}
