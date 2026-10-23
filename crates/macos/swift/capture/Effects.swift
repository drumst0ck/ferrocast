import CoreGraphics
import CoreImage
import Foundation
import Vision

// P6 render effects (T38-T42). ARCHITECTURE: Rust computes every number (`capture::compose`:
// loupe, camera_crop, look, split), this file only draws them. A request without those fields
// takes none of this code, so a project without P6 effects renders byte-identical to P4/P5 (S19).
//
//   loupe        {cx, cy, radius, scale, alpha}   fractions of the screen content, radius = fraction of the
//                                                  OUTPUT width, drawn after the camera bubble
//   camera_crop  {x, y, w, h}                     camera SOURCE pixels, top-left origin
//   look         {file, size, hash}               RGBA f32 LE table written once by Rust (cached here by file+hash)
//   split        {canvas, screen, camera}         output pixel rects from `camera::split_rects`
// `camera.cutout` comes from project.json (CameraSpec): person segmentation, best effort.

struct LoupeFx { var cx: Double, cy: Double, radius: Double, scale: Double, alpha: Double }
struct LookFx: Equatable { var file: String, size: Int, hash: String }
struct SplitFx { var canvas: CGSize, screen: CGRect, camera: CGRect }

/// The P6 fields of a `render.frame` request or an export params line. nil when a present field is malformed.
struct P6Fields {
    var loupe: LoupeFx?
    var cameraCrop: CGRect?
    var look: LookFx?
    var split: SplitFx?

    init?(request req: [String: Any]) {
        func obj(_ k: String) -> [String: Any]?? {
            guard let v = req[k], !(v is NSNull) else { return .some(nil) }
            return (v as? [String: Any]).map { .some($0) }
        }
        func fin(_ d: [String: Any], _ names: [String]) -> [Double]? {
            let v = names.compactMap { num(d[$0]) }
            return v.count == names.count && v.allSatisfy { $0.isFinite } ? v : nil
        }
        func rect(_ d: Any?) -> CGRect? {
            guard let d = d as? [String: Any], let v = fin(d, ["x", "y", "w", "h"]), v[2] > 0, v[3] > 0 else { return nil }
            return CGRect(x: v[0], y: v[1], width: v[2], height: v[3])
        }
        guard case .some(let lp) = obj("loupe"), case .some(let cc) = obj("camera_crop"),
              case .some(let lk) = obj("look"), case .some(let sp) = obj("split") else { return nil }
        if let l = lp {
            guard let v = fin(l, ["cx", "cy", "radius", "scale", "alpha"]), v[2] > 0, v[3] > 0 else { return nil }
            loupe = LoupeFx(cx: v[0], cy: v[1], radius: min(v[2], 0.5), scale: min(max(v[3], 1), 8), alpha: min(max(v[4], 0), 1))
        }
        if let c = cc {
            guard let r = rect(c) else { return nil }
            cameraCrop = r
        }
        if let l = lk {
            guard let f = l["file"] as? String, f.hasPrefix("/"), let n = num(l["size"]), n >= 2, n <= 64,
                  let h = l["hash"] as? String, !h.isEmpty else { return nil }
            look = LookFx(file: f, size: Int(n), hash: h)
        }
        if let s = sp {
            guard let c = s["canvas"] as? [NSNumber], c.count == 2, c[0].doubleValue >= 1, c[1].doubleValue >= 1,
                  let a = rect(s["screen"]), let b = rect(s["camera"]) else { return nil }
            split = SplitFx(canvas: CGSize(width: c[0].doubleValue, height: c[1].doubleValue), screen: a, camera: b)
        }
    }
}

// MARK: - Look (T40)

private var lookFilters: [String: CIFilter] = [:]
private var lookFailures: Set<String> = []

/// The cube filter of a look, cached by path + hash: the table is read from the scratch file once.
private func lookFilter(_ look: LookFx) -> CIFilter? {
    let key = "\(look.file)|\(look.hash)"
    if let f = lookFilters[key] { return f }
    if lookFailures.contains(key) { return nil }
    let want = look.size * look.size * look.size * 4 * MemoryLayout<Float>.size
    guard let data = FileManager.default.contents(atPath: look.file), data.count == want,
          let f = CIFilter(name: "CIColorCubeWithColorSpace") else {
        lookFailures.insert(key)
        log("look table unreadable or the wrong size, camera drawn without the look: \(look.file)")
        return nil
    }
    f.setValue(look.size, forKey: "inputCubeDimension")
    f.setValue(data, forKey: "inputCubeData")
    f.setValue(renderSRGB, forKey: "inputColorSpace")
    lookFilters[key] = f
    return f
}

/// Rust resamples a table with another domain onto 0..1 and blends the intensity in, so the table
/// is applied as it is.
func applyLook(_ img: CIImage, _ look: LookFx) -> CIImage {
    guard let f = lookFilter(look) else { return img }
    f.setValue(img, forKey: kCIInputImageKey)
    return f.outputImage?.cropped(to: img.extent) ?? img
}

// MARK: - Cut-out (T41, beta)

private let personRequest: VNGeneratePersonSegmentationRequest = {
    let r = VNGeneratePersonSegmentationRequest()
    r.qualityLevel = .balanced
    r.outputPixelFormat = kCVPixelFormatType_OneComponent8
    return r
}()
private var maskCache: [(key: String, mask: CIImage?)] = []
private var cutoutFailures = 0

/// Person mask of `image` (gray, white = person) scaled to the extent of `image`, cached by `key`
/// (camera file + frame time + crop) so a repeated `render.frame` is cheap. The mask is copied out of
/// Vision's buffer at once: the request object is reused and would overwrite it. nil on failure.
private func personMask(of image: CIImage, key: String) -> CIImage? {
    if let hit = maskCache.first(where: { $0.key == key }) { return hit.mask }
    var mask: CIImage?
    do {
        let handler = VNImageRequestHandler(ciImage: image, options: [:])
        try handler.perform([personRequest])
        if let pb = personRequest.results?.first?.pixelBuffer {
            let m = CIImage(cvPixelBuffer: pb, options: [.colorSpace: NSNull()])
            let gray = CGColorSpaceCreateDeviceGray()
            if let cg = renderContext.createCGImage(m, from: m.extent, format: .L8, colorSpace: gray) {
                mask = scaled(CIImage(cgImage: cg, options: [.colorSpace: NSNull()]), to: image.extent.size, at: image.extent.origin)
            }
        }
        if mask == nil { throw RenderError("no person mask returned") }
    } catch {
        cutoutFailures += 1
        if cutoutFailures <= 3 { log("camera cut-out failed, drawing the unmasked camera: \(error)") }
    }
    maskCache.append((key, mask))
    if maskCache.count > 4 { maskCache.removeFirst() }
    return mask
}

func applyCutout(_ img: CIImage, key: String) -> CIImage {
    guard let mask = personMask(of: img, key: key) else { return img }
    return img.applyingFilter("CIBlendWithMask", parameters: [
        kCIInputBackgroundImageKey: CIImage(color: .clear).cropped(to: img.extent), kCIInputMaskImageKey: mask,
    ])
}

// MARK: - Camera picture (crop, look, cut-out)

/// Pixel crop (top-left, source pixels) inside `size`, at least 1 px, whole pixels.
func cameraCropRect(_ c: CGRect, size: CGSize) -> CGRect {
    let x0 = max(0, min(Int(c.minX.rounded()), Int(size.width) - 1))
    let y0 = max(0, min(Int(c.minY.rounded()), Int(size.height) - 1))
    let x1 = max(x0 + 1, min(Int(c.maxX.rounded()), Int(size.width)))
    let y1 = max(y0 + 1, min(Int(c.maxY.rounded()), Int(size.height)))
    return CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0)
}

/// The camera frame as it is drawn: cropped to the follow-face rect, with the look, with the
/// background removed. Without any of them it is `frame.image` itself (same bytes as P4).
func cameraPicture(_ frame: TakeFrame, spec: CameraSpec, at ct: Double, fx: FrameFx) -> CIImage {
    var img = frame.image
    var cropKey = "-"
    if let c = fx.cameraCrop {
        let size = CGSize(width: frame.width, height: frame.height)
        let r = cameraCropRect(c, size: size)
        let ci = CGRect(x: r.minX, y: size.height - r.maxY, width: r.width, height: r.height)
        img = img.cropped(to: ci).transformed(by: CGAffineTransform(translationX: -ci.minX, y: -ci.minY))
        cropKey = "\(r.minX),\(r.minY),\(r.width),\(r.height)"
    }
    if spec.cutout {
        img = applyCutout(img, key: "\(spec.url.path)|\(Int64((ct * 1_000_000).rounded()))|\(cropKey)")
    }
    // the look after the mask is computed (the mask wants the original colours), before it is applied
    if let look = fx.look { img = applyLook(img, look) }
    return img
}

// MARK: - Loupe (T38)

private func rgbaBitmap(_ size: CGSize) -> CGContext? {
    CGContext(data: nil, width: max(1, Int(size.width)), height: max(1, Int(size.height)), bitsPerComponent: 8, bytesPerRow: 0,
              space: renderSRGB, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
}

/// Draw the glass magnifier over `base`. `centre` and the radius are in CoreImage pixels (y up);
/// the magnified picture is the SOURCE take frame (so it is crisp, not an enlarged preview) around
/// the focus plus the cursor/ring layers scaled about the centre. Clipped to the card.
///
/// - `focus`: the loupe centre as a fraction of the screen content (take).
/// - `displayPerTakePx`: card pixels per take pixel (the zoom viewport changes it).
func compositeLoupe(_ l: LoupeFx, over base: CIImage, take: TakeFrame, centre: CGPoint, displayPerTakePx k: CGFloat,
                    overlays: [CIImage], card: CGRect, cardRadius: CGFloat, canvas: CGRect) -> CIImage {
    let a = CGFloat(l.alpha)
    guard a > 0.001 else { return base }
    let ref = canvas.width / 1920
    let r = max(4, CGFloat(l.radius) * canvas.width)
    let pad = ceil(r * 0.25 + 8 * ref)
    let box = CGRect(x: centre.x - r - pad, y: centre.y - r - pad, width: 2 * (r + pad), height: 2 * (r + pad)).integral
    let local = CGPoint(x: centre.x - box.minX, y: centre.y - box.minY)

    // 1. what the glass shows: the take around the focus at `scale` x, plus the magnified overlays
    let s = CGFloat(l.scale)
    let tw = CGFloat(take.width), th = CGFloat(take.height)
    let region = CGRect(x: 0, y: 0, width: 2 * r / (s * k), height: 2 * r / (s * k))
    let focusCI = CGPoint(x: CGFloat(l.cx) * tw, y: th - CGFloat(l.cy) * th)
    let regionCI = CGRect(x: focusCI.x - region.width / 2, y: focusCI.y - region.height / 2, width: region.width, height: region.height).integral
    // `scaled` expects an image whose extent starts at 0,0
    let part = take.image.clampedToExtent().cropped(to: regionCI)
        .transformed(by: CGAffineTransform(translationX: -regionCI.minX, y: -regionCI.minY))
    var content = scaled(part, to: CGSize(width: 2 * r, height: 2 * r), at: CGPoint(x: centre.x - r, y: centre.y - r))
    let about = CGAffineTransform(translationX: -centre.x, y: -centre.y).concatenating(CGAffineTransform(scaleX: s, y: s))
        .concatenating(CGAffineTransform(translationX: centre.x, y: centre.y))
    for layer in overlays { content = layer.transformed(by: about).composited(over: content) }

    // 2. coverage: the circle with a soft edge, times alpha, inside the card
    guard let maskCtx = CGContext(data: nil, width: Int(box.width), height: Int(box.height), bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue),
          let shadowCtx = rgbaBitmap(box.size), let rimCtx = rgbaBitmap(box.size),
          let gradient = CGGradient(colorSpace: CGColorSpaceCreateDeviceGray(), colorComponents: [a, 1, 0, 1], locations: [0, 1], count: 2)
    else { return base }
    maskCtx.setFillColor(gray: 0, alpha: 1)
    maskCtx.fill(CGRect(origin: .zero, size: box.size))
    let cardLocal = card.offsetBy(dx: -box.minX, dy: -box.minY)
    maskCtx.addPath(CGPath(roundedRect: cardLocal, cornerWidth: cardRadius, cornerHeight: cardRadius, transform: nil))
    maskCtx.clip()
    let feather = max(1.5, 1.5 * ref)
    maskCtx.drawRadialGradient(gradient, startCenter: local, startRadius: r - feather, endCenter: local, endRadius: r,
                               options: [.drawsBeforeStartLocation])
    guard let maskCG = maskCtx.makeImage() else { return base }
    let mask = CIImage(cgImage: maskCG, options: [.colorSpace: NSNull()]).transformed(by: CGAffineTransform(translationX: box.minX, y: box.minY))

    // 3. drop shadow under the glass (the interior is cleared again) and the rim above it
    let circle = CGRect(x: local.x - r, y: local.y - r, width: 2 * r, height: 2 * r)
    shadowCtx.setShadow(offset: CGSize(width: 0, height: -6 * ref), blur: max(2, 18 * ref),
                        color: CGColor(colorSpace: renderSRGB, components: [0, 0, 0, 0.45 * a])!)
    shadowCtx.setFillColor(CGColor(colorSpace: renderSRGB, components: [0, 0, 0, 1])!)
    shadowCtx.fillEllipse(in: circle)
    shadowCtx.setShadow(offset: .zero, blur: 0, color: nil)
    shadowCtx.setBlendMode(.clear)
    shadowCtx.fillEllipse(in: circle)
    let lw = max(1.2, 1.8 * ref)
    rimCtx.setLineWidth(lw)
    rimCtx.setStrokeColor(CGColor(colorSpace: renderSRGB, components: [0, 0, 0, 0.25 * a])!)
    rimCtx.strokeEllipse(in: circle.insetBy(dx: -lw / 2, dy: -lw / 2))
    rimCtx.setStrokeColor(CGColor(colorSpace: renderSRGB, components: [1, 1, 1, 0.8 * a])!)
    rimCtx.strokeEllipse(in: circle.insetBy(dx: lw / 2, dy: lw / 2))
    // a faint highlight along the upper left inside the rim: reads as glass
    rimCtx.setLineWidth(lw * 1.6)
    rimCtx.setStrokeColor(CGColor(colorSpace: renderSRGB, components: [1, 1, 1, 0.28 * a])!)
    rimCtx.addArc(center: local, radius: r - lw * 3, startAngle: .pi * 0.62, endAngle: .pi * 0.95, clockwise: false)
    rimCtx.strokePath()

    func layer(_ ctx: CGContext) -> CIImage? {
        ctx.makeImage().map { rawImage($0).transformed(by: CGAffineTransform(translationX: box.minX, y: box.minY)) }
    }
    let glass = content.cropped(to: box).applyingFilter("CIBlendWithMask", parameters: [
        kCIInputBackgroundImageKey: CIImage(color: .clear).cropped(to: box), kCIInputMaskImageKey: mask,
    ])
    var out = base
    if let shadow = layer(shadowCtx) { out = shadow.composited(over: out) }
    out = glass.composited(over: out)
    if let rim = layer(rimCtx) { out = rim.composited(over: out) }
    return out.cropped(to: canvas)
}
