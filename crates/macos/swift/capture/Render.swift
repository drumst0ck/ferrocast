import AVFoundation
import CoreGraphics
import CoreImage
import Foundation
import ImageIO
import Metal
import UniformTypeIdentifiers

// `render.frame`: one still of a project, styled like the editor preview (T24).
// Style only: background, padding, rounded corners, drop shadow, output aspect. The camera
// transform, cursor, click ring and motion blur come with T25. P6 fields of project.json
// (loupes, looks, split, ...) are ignored here, so a project without them renders exactly as before.
//
// Units: `style.radius`, `shadow.blur` and `shadow.dy` are authored for a 1920 px wide output and
// scale with the requested width; `style.padding` is a fraction of the output width per side.
// `t` is the source time of the take in seconds (the caller maps output time with `timeline`).

private func num(_ v: Any?) -> Double? { (v as? NSNumber)?.doubleValue }

/// Style as stored in project.json, with the same defaults as `capture::project::Style`.
struct RenderStyle {
    var aspect = "16:9"
    var padding = 0.06
    var radius = 18.0
    var shadowBlur = 40.0
    var shadowOpacity = 0.5
    var shadowDy = 16.0
    var bgKind = "gradient"
    var bgStops = ["#1f1c2c", "#928dab"]
    var bgAngle = 135.0

    init(json: [String: Any]) {
        guard let s = json["style"] as? [String: Any] else { return }
        if let a = s["aspect"] as? String { aspect = a }
        if let v = num(s["padding"]) { padding = v }
        if let v = num(s["radius"]) { radius = v }
        if let sh = s["shadow"] as? [String: Any] {
            if let v = num(sh["blur"]) { shadowBlur = v }
            if let v = num(sh["opacity"]) { shadowOpacity = v }
            if let v = num(sh["dy"]) { shadowDy = v }
        }
        if let bg = s["background"] as? [String: Any] {
            if let v = bg["kind"] as? String { bgKind = v }
            if let v = bg["stops"] as? [String] { bgStops = v }
            if let v = num(bg["angle"]) { bgAngle = v }
        }
    }
}

/// Pixel geometry of one render, top-left origin.
struct RenderGeometry: Equatable {
    var canvas: CGSize
    var frame: CGRect
    var radius: CGFloat
    var shadowSigma: CGFloat
    var shadowDy: CGFloat
}

/// Output aspect ("16:9", "4:3", "9:16", "1:1"); anything else follows the source.
func renderAspect(_ spec: String, source: CGSize) -> Double {
    let parts = spec.split(whereSeparator: { $0 == ":" || $0 == "/" }).compactMap { Double($0) }
    if parts.count == 2, parts[0] > 0, parts[1] > 0 { return parts[0] / parts[1] }
    return Double(source.width / max(source.height, 1))
}

func renderGeometry(_ style: RenderStyle, source: CGSize, width: Int) -> RenderGeometry {
    let w = Double(width)
    let h = max(1, (w / renderAspect(style.aspect, source: source)).rounded())
    let ref = w / 1920
    let pad = min(max(style.padding, 0), 0.45) * w
    let iw = max(1, w - 2 * pad), ih = max(1, h - 2 * pad)
    let s = min(iw / Double(source.width), ih / Double(source.height))
    let fw = max(1, (Double(source.width) * s).rounded()), fh = max(1, (Double(source.height) * s).rounded())
    let frame = CGRect(x: ((w - fw) / 2).rounded(), y: ((h - fh) / 2).rounded(), width: fw, height: fh)
    return RenderGeometry(
        canvas: CGSize(width: w, height: h), frame: frame,
        radius: CGFloat(min(max(style.radius, 0) * ref, min(fw, fh) / 2)),
        shadowSigma: CGFloat(max(style.shadowBlur, 0) * ref / 2),
        shadowDy: CGFloat(style.shadowDy * ref))
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
func renderHexColor(_ s: String) -> CGColor? {
    var h = s.trimmingCharacters(in: .whitespaces)
    if h.hasPrefix("#") { h.removeFirst() }
    if h.count == 3 { h = h.map { "\($0)\($0)" }.joined() }
    guard h.count == 6 || h.count == 8, let v = UInt64(h, radix: 16) else { return nil }
    let hasAlpha = h.count == 8
    let c = { (shift: UInt64) in CGFloat((v >> shift) & 0xFF) / 255 }
    return CGColor(colorSpace: renderSRGB, components: hasAlpha ? [c(24), c(16), c(8), c(0)] : [c(16), c(8), c(0), 1])
}

let renderSRGB = CGColorSpace(name: CGColorSpace.sRGB)!

/// Bundled wallpapers: gradients generated in code, no image assets.
private let wallpapers: [(name: String, stops: [String], angle: Double)] = [
    ("aurora", ["#0f2027", "#2c5364", "#42e695"], 135), ("dusk", ["#2b1055", "#d53369", "#ff9a5a"], 160),
    ("ocean", ["#0f4c81", "#1fa2ff", "#a6ffcb"], 120), ("graphite", ["#141414", "#3a3a3c"], 135),
    ("sand", ["#f5e6c8", "#d9a066"], 150), ("lilac", ["#e0c3fc", "#8ec5fc"], 135),
    ("forest", ["#0b3d2e", "#2e8b57", "#b6e388"], 145), ("rose", ["#ffdde1", "#ee9ca7"], 135),
]

private func gradientImage(stops: [String], angle: Double, size: CGSize) -> CGImage? {
    let colors = stops.compactMap(renderHexColor)
    guard !colors.isEmpty, let ctx = renderBitmap(size: size) else { return nil }
    if colors.count == 1 {
        ctx.setFillColor(colors[0])
        ctx.fill(CGRect(origin: .zero, size: size))
    } else if let g = CGGradient(colorsSpace: renderSRGB, colors: colors as CFArray, locations: nil) {
        // CSS convention: 0° points up, 90° right. CoreGraphics is y-up, so the vector is (sin, cos).
        let a = angle * .pi / 180
        let dx = sin(a), dy = cos(a)
        let len = abs(Double(size.width) * dx) + abs(Double(size.height) * dy)
        let cx = Double(size.width) / 2, cy = Double(size.height) / 2
        ctx.drawLinearGradient(g, start: CGPoint(x: cx - dx * len / 2, y: cy - dy * len / 2),
                               end: CGPoint(x: cx + dx * len / 2, y: cy + dy * len / 2), options: [])
    }
    return ctx.makeImage()
}

private func renderBitmap(size: CGSize) -> CGContext? {
    CGContext(data: nil, width: Int(size.width), height: Int(size.height), bitsPerComponent: 8, bytesPerRow: 0,
              space: renderSRGB, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
}

/// Rounded-rect coverage (white inside) on a canvas-sized gray bitmap, `rect` in top-left pixels.
private func maskImage(canvas: CGSize, rect: CGRect, radius: CGFloat) -> CGImage? {
    guard let ctx = CGContext(data: nil, width: Int(canvas.width), height: Int(canvas.height), bitsPerComponent: 8,
                              bytesPerRow: 0, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)
    else { return nil }
    ctx.setFillColor(gray: 0, alpha: 1)
    ctx.fill(CGRect(origin: .zero, size: canvas))
    ctx.setFillColor(gray: 1, alpha: 1)
    let flipped = CGRect(x: rect.minX, y: canvas.height - rect.maxY, width: rect.width, height: rect.height)
    ctx.addPath(CGPath(roundedRect: flipped, cornerWidth: radius, cornerHeight: radius, transform: nil))
    ctx.fillPath()
    return ctx.makeImage()
}

private func rawImage(_ cg: CGImage) -> CIImage { CIImage(cgImage: cg, options: [.colorSpace: NSNull()]) }

private let renderContext: CIContext = {
    let opts: [CIContextOption: Any] = [.workingColorSpace: renderSRGB, .outputColorSpace: renderSRGB]
    if MTLCreateSystemDefaultDevice() != nil { return CIContext(options: opts) }
    var cpu = opts
    cpu[.useSoftwareRenderer] = true
    return CIContext(options: cpu)
}()

private func scaled(_ img: CIImage, to size: CGSize, at origin: CGPoint) -> CIImage {
    let src = img.extent
    let f = CIFilter(name: "CILanczosScaleTransform")!
    f.setValue(img, forKey: kCIInputImageKey)
    f.setValue(size.height / src.height, forKey: kCIInputScaleKey)
    f.setValue((size.width / src.width) / (size.height / src.height), forKey: kCIInputAspectRatioKey)
    let out = f.outputImage!.cropped(to: CGRect(x: src.minX, y: src.minY, width: size.width, height: size.height))
    return out.transformed(by: CGAffineTransform(translationX: origin.x - out.extent.minX, y: origin.y - out.extent.minY))
}

private func blurred(_ img: CIImage, sigma: CGFloat, within rect: CGRect) -> CIImage {
    guard sigma > 0.01 else { return img.cropped(to: rect) }
    return img.clampedToExtent().applyingFilter("CIGaussianBlur", parameters: [kCIInputRadiusKey: sigma]).cropped(to: rect)
}

/// Aspect-fill `img` over the canvas rect (CI coordinates).
private func coverCanvas(_ img: CIImage, canvas: CGRect) -> CIImage {
    let s = max(canvas.width / img.extent.width, canvas.height / img.extent.height)
    let size = CGSize(width: img.extent.width * s, height: img.extent.height * s)
    let placed = scaled(img, to: size, at: CGPoint(x: canvas.midX - size.width / 2, y: canvas.midY - size.height / 2))
    return placed.cropped(to: canvas)
}

private func backgroundImage(_ style: RenderStyle, take: CGImage, project: String, geo: RenderGeometry, canvas: CGRect) -> CIImage {
    func gradient(_ stops: [String], _ angle: Double) -> CIImage? {
        gradientImage(stops: stops, angle: angle, size: geo.canvas).map(rawImage)
    }
    switch style.bgKind {
    case "color":
        if let c = style.bgStops.first, gradient([c], 0) != nil { return gradient([c], 0)! }
    case "wallpaper":
        let key = style.bgStops.first ?? ""
        let w = wallpapers.first { $0.name == key } ?? wallpapers[(Int(key) ?? 0) % wallpapers.count]
        if let g = gradient(w.stops, w.angle) { return g }
    case "image":
        if let path = style.bgStops.first, !path.isEmpty {
            let full = path.hasPrefix("/") ? path : (project as NSString).appendingPathComponent(path)
            if !path.contains(".."), let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: full) as CFURL, nil),
               let img = CGImageSourceCreateImageAtIndex(src, 0, nil) {
                return coverCanvas(CIImage(cgImage: img), canvas: canvas)
            }
            log("render.frame: background image unreadable, using the default gradient: \(path)")
        }
    case "blur":
        // The frame itself, covering the canvas, blurred (same idea as `frame blur` for stills).
        let sigma = 40 * geo.canvas.width / 1920
        return blurred(coverCanvas(rawImage(take), canvas: canvas), sigma: sigma, within: canvas)
    default:
        if let g = gradient(style.bgStops, style.bgAngle) { return g }
    }
    return gradient(RenderStyle(json: [:]).bgStops, 135) ?? CIImage(color: .black).cropped(to: canvas)
}

/// Compose the styled still for one take frame.
func composeStyledFrame(take: CGImage, style: RenderStyle, project: String, width: Int) -> CGImage? {
    let geo = renderGeometry(style, source: CGSize(width: take.width, height: take.height), width: width)
    let canvas = CGRect(origin: .zero, size: geo.canvas)
    // CoreImage is y-up; the geometry is top-left.
    let frameCI = CGRect(x: geo.frame.minX, y: geo.canvas.height - geo.frame.maxY, width: geo.frame.width, height: geo.frame.height)

    guard let maskCG = maskImage(canvas: geo.canvas, rect: geo.frame, radius: geo.radius) else { return nil }
    let mask = rawImage(maskCG)
    let clear = CIImage(color: .clear).cropped(to: canvas)

    let picture = scaled(CIImage(cgImage: take), to: frameCI.size, at: frameCI.origin)
    let rounded = picture.applyingFilter("CIBlendWithMask", parameters: [kCIInputBackgroundImageKey: clear, kCIInputMaskImageKey: mask])

    var result = backgroundImage(style, take: take, project: project, geo: geo, canvas: canvas)
    if style.shadowOpacity > 0, geo.shadowSigma > 0.01 || geo.shadowDy != 0 {
        let shape = mask.applyingFilter("CIMaskToAlpha")
        let opacity = CGFloat(min(max(style.shadowOpacity, 0), 1))
        let shadow = blurred(shape, sigma: geo.shadowSigma, within: canvas.insetBy(dx: -4 * geo.shadowSigma, dy: -4 * geo.shadowSigma))
            // CIMaskToAlpha yields white; make it black and scale the alpha by the opacity.
            .applyingFilter("CIColorMatrix", parameters: [
                "inputRVector": CIVector(x: 0, y: 0, z: 0, w: 0), "inputGVector": CIVector(x: 0, y: 0, z: 0, w: 0),
                "inputBVector": CIVector(x: 0, y: 0, z: 0, w: 0), "inputAVector": CIVector(x: 0, y: 0, z: 0, w: opacity),
            ])
            .transformed(by: CGAffineTransform(translationX: 0, y: -geo.shadowDy))
        result = shadow.composited(over: result)
    }
    result = rounded.composited(over: result).cropped(to: canvas)
    return renderContext.createCGImage(result, from: canvas, format: .RGBA8, colorSpace: renderSRGB)
}

// MARK: - Take access

private final class ResultBox<T>: @unchecked Sendable { var value: Result<T, Error>? }

private func blocking<T>(_ op: @escaping @Sendable () async throws -> T) throws -> T {
    let box = ResultBox<T>()
    let sem = DispatchSemaphore(value: 0)
    Task.detached {
        do { box.value = .success(try await op()) } catch { box.value = .failure(error) }
        sem.signal()
    }
    sem.wait()
    return try box.value!.get()
}

/// The take frame at exactly `t` (clamped into the movie): zero tolerance, so a scrubbed time
/// returns the frame covering it rather than a nearby keyframe.
private func takeFrame(at url: URL, t: Double) throws -> CGImage {
    let asset = AVURLAsset(url: url)
    let (duration, fps) = try blocking {
        let d = try await asset.load(.duration).seconds
        let f = try await asset.loadTracks(withMediaType: .video).first?.load(.nominalFrameRate) ?? 30
        return (d, Double(f))
    }
    guard duration.isFinite, duration > 0 else { throw RenderError("take has no duration") }
    let last = max(0, duration - 0.5 / max(fps, 1))
    let at = CMTime(seconds: min(max(t, 0), last), preferredTimescale: 60000)
    let gen = AVAssetImageGenerator(asset: asset)
    gen.appliesPreferredTrackTransform = true
    gen.requestedTimeToleranceBefore = .zero
    gen.requestedTimeToleranceAfter = .zero
    let sem = DispatchSemaphore(value: 0)
    var image: CGImage?
    var failure: Error?
    gen.generateCGImagesAsynchronously(forTimes: [NSValue(time: at)]) { _, img, _, result, err in
        if result == .succeeded { image = img } else { failure = err ?? RenderError("frame unavailable") }
        sem.signal()
    }
    if sem.wait(timeout: .now() + 8) == .timedOut {
        gen.cancelAllCGImageGeneration()
        throw RenderError("take frame timed out")
    }
    guard let image else { throw failure ?? RenderError("frame unavailable") }
    return image
}

struct RenderError: Error {
    let message: String
    init(_ m: String) { message = m }
}

// MARK: - Cache and output

private struct FrameKey: Hashable {
    let project: String
    let mtime: Double
    let takeMtime: Double
    let t: Int64
    let width: Int
}

private struct CachedFrame {
    let png: Data
    let size: [Int]
}

/// Small bounded cache of encoded frames (insertion order = eviction order).
private var frameCache: [FrameKey: CachedFrame] = [:]
private var frameCacheOrder: [FrameKey] = []
private let frameCacheLimit = 12
private let frameCacheBytes = 48 * 1024 * 1024

private func cacheStore(_ key: FrameKey, _ value: CachedFrame) {
    frameCache[key] = value
    frameCacheOrder.removeAll { $0 == key }
    frameCacheOrder.append(key)
    while frameCacheOrder.count > frameCacheLimit || (frameCacheOrder.count > 1 && frameCache.values.reduce(0) { $0 + $1.png.count } > frameCacheBytes) {
        frameCache[frameCacheOrder.removeFirst()] = nil
    }
}

private func mtime(_ path: String) -> Double {
    ((try? FileManager.default.attributesOfItem(atPath: path)[.modificationDate]) as? Date)?.timeIntervalSince1970 ?? 0
}

/// Atomic write: temp file next to `out`, then rename.
private func writeAtomically(_ data: Data, to out: String) -> Bool {
    let fm = FileManager.default
    let tmp = out + ".partial"
    try? fm.createDirectory(atPath: (out as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
    do {
        try data.write(to: URL(fileURLWithPath: tmp))
        if fm.fileExists(atPath: out) { try fm.removeItem(atPath: out) }
        try fm.moveItem(atPath: tmp, toPath: out)
        return true
    } catch {
        try? fm.removeItem(atPath: tmp)
        return false
    }
}

private func encodePNG(_ image: CGImage) -> Data? {
    let data = NSMutableData()
    guard let dest = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else { return nil }
    CGImageDestinationAddImage(dest, image, nil)
    return CGImageDestinationFinalize(dest) ? data as Data : nil
}

/// Preview frames older than 60 s are deleted (plan: "Protocol").
private func pruneFrameCache(project: String, keep: String) {
    let dir = (project as NSString).appendingPathComponent(".cache")
    let fm = FileManager.default
    for name in (try? fm.contentsOfDirectory(atPath: dir)) ?? [] where name.hasPrefix("f-") && name.hasSuffix(".png") {
        let path = (dir as NSString).appendingPathComponent(name)
        if path != keep, Date().timeIntervalSince1970 - mtime(path) > 60 { try? fm.removeItem(atPath: path) }
    }
}

/// `render.frame`: {project, t, width, out?} -> {path, size, ms, cached}.
func handleRenderFrame(_ id: Any?, _ req: [String: Any]) {
    let started = DispatchTime.now()
    guard let project = req["project"] as? String, project.hasPrefix("/"),
          let t = num(req["t"]), t.isFinite, let w = num(req["width"]), w >= 16, w <= 8192 else {
        fail(id, "invalid", "render.frame needs an absolute project, a finite t and width 16...8192")
        return
    }
    let width = Int(w)
    let out = (req["out"] as? String) ?? (project as NSString).appendingPathComponent(".cache/f-\(Int(max(t, 0) * 1000))-\(width).png")
    guard out.hasPrefix("/") else {
        fail(id, "invalid", "out must be an absolute path")
        return
    }
    let projectFile = (project as NSString).appendingPathComponent("project.json")
    guard let raw = FileManager.default.contents(atPath: projectFile) else {
        fail(id, "not-found", "project.json not found")
        return
    }
    guard let json = (try? JSONSerialization.jsonObject(with: raw)) as? [String: Any] else {
        fail(id, "invalid", "project.json is not valid")
        return
    }
    let file = ((json["sources"] as? [String: Any])?["screen"] as? [String: Any])?["file"] as? String ?? "screen.mov"
    guard !file.isEmpty, !file.contains("/"), !file.contains("..") else {
        fail(id, "invalid", "sources.screen.file must be a plain file name")
        return
    }
    let takePath = (project as NSString).appendingPathComponent(file)
    guard FileManager.default.fileExists(atPath: takePath) else {
        fail(id, "not-found", "\(file) not found")
        return
    }

    let key = FrameKey(project: project, mtime: mtime(projectFile), takeMtime: mtime(takePath),
                       t: Int64((max(t, 0) * 1_000_000).rounded()), width: width)
    let ms = { Double(DispatchTime.now().uptimeNanoseconds - started.uptimeNanoseconds) / 1_000_000 }
    if let hit = frameCache[key] {
        guard writeAtomically(hit.png, to: out) else { fail(id, "internal", "could not write \(out)"); return }
        reply(id, ["path": out, "size": hit.size, "ms": (ms() * 10).rounded() / 10, "cached": true])
        return
    }
    do {
        let take = try takeFrame(at: URL(fileURLWithPath: takePath), t: t)
        guard let image = composeStyledFrame(take: take, style: RenderStyle(json: json), project: project, width: width),
              let png = encodePNG(image) else {
            fail(id, "internal", "could not compose the frame")
            return
        }
        guard writeAtomically(png, to: out) else { fail(id, "internal", "could not write \(out)"); return }
        let size = [image.width, image.height]
        cacheStore(key, CachedFrame(png: png, size: size))
        pruneFrameCache(project: project, keep: out)
        reply(id, ["path": out, "size": size, "ms": (ms() * 10).rounded() / 10, "cached": false])
    } catch let e as RenderError {
        fail(id, "internal", e.message)
    } catch {
        fail(id, "internal", "render.frame failed: \(error.localizedDescription)")
    }
}
