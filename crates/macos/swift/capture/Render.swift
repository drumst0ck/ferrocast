import AVFoundation
import CoreGraphics
import CoreImage
import Foundation
import ImageIO
import Metal
import UniformTypeIdentifiers

// `render.frame`: one still of a project, styled like the editor preview (T24).
// Style: background, padding, rounded corners, drop shadow, output aspect. T25 adds, from the optional
// `camera`, `cursor`, `ring` and `blur` request fields (all maths is evaluated by `capture::compose`
// in Rust, Swift only draws): a crop of the SOURCE frame to the viewport before styling, the cursor
// sprite, the click ring and a directional motion blur. A request without those fields renders exactly
// as T24 did. P6 fields of project.json (loupes, looks, split, ...) are ignored here, so a project
// without them renders exactly as before.
//
// Units: `style.radius`, `shadow.blur` and `shadow.dy` are authored for a 1920 px wide output and
// scale with the requested width; `style.padding` is a fraction of the output width per side.
// `t` is the source time of the take in seconds (the caller maps output time with `timeline`).
//
// Performance (S15, measured on a real 2940x1912 HEVC take, 960 px preview): the editor asks for
// `format:"raw"` frames, which skip the PNG encode here and decode in the app, and whose take frames
// come from a sequential AVAssetReader while the times are consecutive (Decode.swift). The default
// PNG path is untouched (generator decode, same graph, same bytes: the smoke golden checks it). The
// mask and the background + shadow graph are kept between frames with the same style and geometry.

func num(_ v: Any?) -> Double? { (v as? NSNumber)?.doubleValue }

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

/// `height` (export only) pins the canvas height; without it the height follows the aspect, as in `render.frame`.
///
/// `area` (P6 split layout, top-left pixels) fits the card into that part of the canvas, with the
/// same padding, instead of into the whole canvas; without it the geometry is exactly as before.
func renderGeometry(_ style: RenderStyle, source: CGSize, width: Int, height: Int? = nil, area: CGRect? = nil) -> RenderGeometry {
    let w = Double(width)
    let h = height.map { Double(max(1, $0)) } ?? max(1, (w / renderAspect(style.aspect, source: source)).rounded())
    let ref = w / 1920
    let pad = min(max(style.padding, 0), 0.45) * w
    let (ox, oy, aw, ah) = area.map { (Double($0.minX), Double($0.minY), Double($0.width), Double($0.height)) } ?? (0, 0, w, h)
    let iw = max(1, aw - 2 * pad), ih = max(1, ah - 2 * pad)
    let s = min(iw / Double(source.width), ih / Double(source.height))
    let fw = max(1, (Double(source.width) * s).rounded()), fh = max(1, (Double(source.height) * s).rounded())
    let frame = CGRect(x: ox + ((aw - fw) / 2).rounded(), y: oy + ((ah - fh) / 2).rounded(), width: fw, height: fh)
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
func maskImage(canvas: CGSize, rect: CGRect, radius: CGFloat) -> CGImage? {
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

func rawImage(_ cg: CGImage) -> CIImage { CIImage(cgImage: cg, options: [.colorSpace: NSNull()]) }

struct BackdropKey: Equatable {
    let style: String
    let geo: RenderGeometry
    let project: String

    init(style s: RenderStyle, geo: RenderGeometry, project: String) {
        // every field that changes the background or the shadow
        var text = "\(s.bgKind)|\(s.bgStops)|\(s.bgAngle)|\(s.shadowOpacity)"
        if s.bgKind == "image", let path = s.bgStops.first {
            let full = path.hasPrefix("/") ? path : (project as NSString).appendingPathComponent(path)
            text += "|\(mtime(full))"
        }
        style = text
        self.geo = geo
        self.project = project
    }
}

struct Backdrop {
    let key: BackdropKey
    let mask: CIImage
    let backdrop: CIImage?
}

private var backdropCache: Backdrop?

let renderContext: CIContext = {
    let opts: [CIContextOption: Any] = [.workingColorSpace: renderSRGB, .outputColorSpace: renderSRGB]
    if MTLCreateSystemDefaultDevice() != nil { return CIContext(options: opts) }
    var cpu = opts
    cpu[.useSoftwareRenderer] = true
    return CIContext(options: cpu)
}()

func scaled(_ img: CIImage, to size: CGSize, at origin: CGPoint) -> CIImage {
    let src = img.extent
    let f = CIFilter(name: "CILanczosScaleTransform")!
    f.setValue(img, forKey: kCIInputImageKey)
    f.setValue(size.height / src.height, forKey: kCIInputScaleKey)
    f.setValue((size.width / src.width) / (size.height / src.height), forKey: kCIInputAspectRatioKey)
    let out = f.outputImage!.cropped(to: CGRect(x: src.minX, y: src.minY, width: size.width, height: size.height))
    return out.transformed(by: CGAffineTransform(translationX: origin.x - out.extent.minX, y: origin.y - out.extent.minY))
}

func blurred(_ img: CIImage, sigma: CGFloat, within rect: CGRect) -> CIImage {
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

private func backgroundImage(_ style: RenderStyle, take: TakeFrame, project: String, geo: RenderGeometry, canvas: CGRect) -> CIImage {
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
        return blurred(coverCanvas(take.raw(), canvas: canvas), sigma: sigma, within: canvas)
    default:
        if let g = gradient(style.bgStops, style.bgAngle) { return g }
    }
    return gradient(RenderStyle(json: [:]).bgStops, 135) ?? CIImage(color: .black).cropped(to: canvas)
}

/// The optional per-frame fields of `render.frame` (normalised to the recorded display, top-left origin).
struct FrameFx {
    struct Cursor { var x: Double, y: Double, kind: String, size: Double, opacity: Double }
    struct Ring { var x: Double, y: Double, radius: Double, alpha: Double }
    struct Blur { var samples: Int, dir: [Double], length: Double, scene: Bool }
    var camera: CGRect?
    var cursor: Cursor?
    var ring: Ring?
    var blur: Blur?
    /// P6 (Effects.swift): loupe, camera crop, look table and split rectangles, all computed by Rust.
    var loupe: LoupeFx?
    var cameraCrop: CGRect?
    var look: LookFx?
    var split: SplitFx?
    /// Canonical text of the fields, part of the frame cache key.
    var key = ""

    static let none = FrameFx()

    var isNone: Bool { camera == nil && cursor == nil && ring == nil && blur == nil }

    /// nil when a present field is malformed.
    init?(request req: [String: Any]) {
        func obj(_ k: String) -> [String: Any]?? {
            guard let v = req[k], !(v is NSNull) else { return .some(nil) }
            return (v as? [String: Any]).map { .some($0) }
        }
        func fin(_ d: [String: Any], _ names: [String]) -> [Double]? {
            let v = names.compactMap { num(d[$0]) }
            return v.count == names.count && v.allSatisfy { $0.isFinite } ? v : nil
        }
        guard case .some(let cam) = obj("camera"), case .some(let cur) = obj("cursor"),
              case .some(let rng) = obj("ring"), case .some(let blr) = obj("blur") else { return nil }
        if let c = cam {
            guard let v = fin(c, ["x", "y", "w", "h"]), v[2] > 0, v[3] > 0 else { return nil }
            // S15: the viewport never leaves the display, whatever the caller sent.
            let w = min(v[2], 1), h = min(v[3], 1)
            camera = CGRect(x: min(max(v[0], 0), 1 - w), y: min(max(v[1], 0), 1 - h), width: w, height: h)
        }
        if let c = cur {
            guard let v = fin(c, ["x", "y", "size", "opacity"]), v[2] > 0 else { return nil }
            cursor = Cursor(x: v[0], y: v[1], kind: (c["kind"] as? String) ?? "arrow", size: min(v[2], 8), opacity: min(max(v[3], 0), 1))
        }
        if let r = rng {
            guard let v = fin(r, ["x", "y", "radius", "alpha"]) else { return nil }
            ring = Ring(x: v[0], y: v[1], radius: max(v[2], 0), alpha: min(max(v[3], 0), 1))
        }
        if let b = blr {
            guard let v = fin(b, ["length"]), let d = b["dir"] as? [NSNumber], d.count == 2,
                  let n = num(b["samples"]) else { return nil }
            blur = Blur(samples: Int(min(max(n, 0), 32)), dir: d.map { $0.doubleValue }, length: max(v[0], 0), scene: (b["scene"] as? Bool) ?? false)
        }
        guard let p6 = P6Fields(request: req) else { return nil }
        loupe = p6.loupe
        cameraCrop = p6.cameraCrop
        look = p6.look
        split = p6.split
        var parts: [String: Any] = [:]
        for k in ["camera", "cursor", "ring", "blur", "loupe", "camera_crop", "look", "split"] { if let v = req[k], !(v is NSNull) { parts[k] = v } }
        if !parts.isEmpty, let d = try? JSONSerialization.data(withJSONObject: parts, options: [.sortedKeys]) {
            key = String(decoding: d, as: UTF8.self)
        }
    }

    private init() {}
}

/// Pixel crop of the take for a normalised viewport, top-left origin, at least 1 px, inside the take.
private func cropRect(_ cam: CGRect, take: CGSize) -> CGRect {
    let x0 = max(0, min(Int((cam.minX * take.width).rounded()), Int(take.width) - 1))
    let y0 = max(0, min(Int((cam.minY * take.height).rounded()), Int(take.height) - 1))
    let x1 = max(x0 + 1, min(Int((cam.maxX * take.width).rounded()), Int(take.width)))
    let y1 = max(y0 + 1, min(Int((cam.maxY * take.height).rounded()), Int(take.height)))
    return CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0)
}

/// Cursor sprite and click ring as separate transparent layers the size of the picture. `view` is
/// the normalised viewport actually shown; `ref` scales authored sizes like the rest of the style.
private func overlayLayers(_ fx: FrameFx, view: CGRect, picture: CGSize, ref: CGFloat) -> (cursor: CIImage?, ring: CIImage?) {
    func map(_ x: Double, _ y: Double) -> CGPoint {
        CGPoint(x: CGFloat((x - view.minX) / view.width) * picture.width, y: CGFloat((y - view.minY) / view.height) * picture.height)
    }
    var ring: CIImage?
    if let r = fx.ring, r.alpha > 0 {
        let c = map(r.x, r.y), radius = CGFloat(r.radius / view.width) * picture.width
        ring = drawnLayer(size: picture) { drawClickRing($0, center: c, radius: radius, alpha: CGFloat(r.alpha), line: max(1, 3 * ref)) }
    }
    var cursor: CIImage?
    if let c = fx.cursor, c.opacity > 0 {
        let at = map(c.x, c.y)
        cursor = drawnLayer(size: picture) { drawCursorSprite($0, kind: c.kind, at: at, scale: CGFloat(c.size) * ref, opacity: CGFloat(c.opacity)) }
    }
    return (cursor, ring)
}

/// Compose the styled still for one take frame (PNG path of `render.frame`).
func composeStyledFrame(take: TakeFrame, style: RenderStyle, project: String, width: Int, fx: FrameFx = .none,
                        camera: CameraDraw? = nil) -> CGImage? {
    guard let (result, canvas) = composeStyledImage(take: take, style: style, project: project, width: width, fx: fx, camera: camera) else { return nil }
    return renderContext.createCGImage(result, from: canvas, format: .RGBA8, colorSpace: renderSRGB)
}

/// The same composition rasterised into `buffer` as tightly packed RGBA8 (premultiplied, sRGB, top row
/// first): the raw hand-off of `render.frame` (`format:"raw"`). The canvas is opaque (the background
/// covers it), so premultiplied and straight alpha are the same bytes. Same CoreImage graph and the
/// same pixel format as the PNG path: the smoke check compares both pixel for pixel.
func composeStyledRaw(take: TakeFrame, style: RenderStyle, project: String, width: Int, fx: FrameFx = .none,
                      camera: CameraDraw? = nil, into buffer: inout [UInt8]) -> (width: Int, height: Int)? {
    guard let (result, canvas) = composeStyledImage(take: take, style: style, project: project, width: width, fx: fx, camera: camera) else { return nil }
    let w = Int(canvas.width), h = Int(canvas.height)
    if buffer.count != w * h * 4 { buffer = [UInt8](repeating: 0, count: w * h * 4) }
    buffer.withUnsafeMutableBytes {
        renderContext.render(result, toBitmap: $0.baseAddress!, rowBytes: w * 4, bounds: canvas, format: .RGBA8, colorSpace: renderSRGB)
    }
    return (w, h)
}

/// The composition shared by `render.frame` and the MP4 export (`Export.swift`): one lazy
/// CoreImage graph over `canvas` (top-left pixels, y-up CI space). Callers rasterise it.
func composeStyledImage(take: TakeFrame, style: RenderStyle, project: String, width: Int, height: Int? = nil, fx: FrameFx = .none,
                       camera: CameraDraw? = nil) -> (CIImage, CGRect)? {
    // P6 split: the card lives in the screen part of the canvas (rects from `camera::split_rects`, scaled
    // when this canvas differs from the one Rust planned for)
    var splitRects: (screen: CGRect, camera: CGRect)?
    if let sp = fx.split, let cam = camera, cam.spec.isSplit {
        let probe = renderGeometry(style, source: CGSize(width: take.width, height: take.height), width: width, height: height)
        let sx = probe.canvas.width / sp.canvas.width, sy = probe.canvas.height / sp.canvas.height
        let m = { (r: CGRect) in CGRect(x: r.minX * sx, y: r.minY * sy, width: r.width * sx, height: r.height * sy) }
        splitRects = (m(sp.screen), m(sp.camera))
    }
    let geo = renderGeometry(style, source: CGSize(width: take.width, height: take.height), width: width, height: height,
                             area: splitRects?.screen)
    let canvas = CGRect(origin: .zero, size: geo.canvas)
    // CoreImage is y-up; the geometry is top-left.
    let frameCI = CGRect(x: geo.frame.minX, y: geo.canvas.height - geo.frame.maxY, width: geo.frame.width, height: geo.frame.height)

    // The mask and the backdrop (background + shadow) depend only on the style and the geometry: a
    // preview or an export asks for hundreds of frames with the same ones, so the CoreImage graph
    // pieces are kept (measured on a real take: export 1080p/30 55 -> 75 fps, raw preview ~1 ms per
    // frame). They are the same lazy graphs the uncached path builds, so the pixels are identical.
    // `blur` backgrounds follow the frame and are never kept.
    let key = BackdropKey(style: style, geo: geo, project: project)
    let hit = backdropCache.flatMap { $0.key == key ? $0 : nil }
    let mask: CIImage
    if let hit { mask = hit.mask } else {
        guard let maskCG = maskImage(canvas: geo.canvas, rect: geo.frame, radius: geo.radius) else { return nil }
        mask = rawImage(maskCG)
    }
    let clear = CIImage(color: .clear).cropped(to: canvas)

    var picture: CIImage
    if let cam = fx.camera {
        // Camera: crop the SOURCE frame to the viewport (pixel aligned) and scale it to the frame.
        let takeSize = CGSize(width: take.width, height: take.height)
        let r = cropRect(cam, take: takeSize)
        let ci = CGRect(x: r.minX, y: takeSize.height - r.maxY, width: r.width, height: r.height)
        let part = take.image.cropped(to: ci).transformed(by: CGAffineTransform(translationX: -ci.minX, y: -ci.minY))
        picture = scaled(part, to: frameCI.size, at: frameCI.origin)
    } else {
        picture = scaled(take.image, to: frameCI.size, at: frameCI.origin)
    }
    var viewport = CGRect(x: 0, y: 0, width: 1, height: 1)
    var overlayLayers_: [CIImage] = []
    if !fx.isNone {
        let view = fx.camera.map { c -> CGRect in
            let r = cropRect(c, take: CGSize(width: take.width, height: take.height))
            return CGRect(x: r.minX / CGFloat(take.width), y: r.minY / CGFloat(take.height),
                          width: r.width / CGFloat(take.width), height: r.height / CGFloat(take.height))
        } ?? CGRect(x: 0, y: 0, width: 1, height: 1)
        let ref = CGFloat(geo.canvas.width / 1920)
        var layers = overlayLayers(fx, view: view, picture: geo.frame.size, ref: ref)
        let place = CGAffineTransform(translationX: frameCI.minX, y: frameCI.minY)
        if let b = fx.blur, b.samples > 1 {
            let len = b.length * Double(frameCI.width)
            if b.scene {
                picture = motionBlurred(picture, dir: b.dir, length: len, samples: b.samples, clamped: true)
            } else if let c = layers.cursor {
                layers.cursor = motionBlurred(c, dir: b.dir, length: len, samples: b.samples, clamped: false)
            }
        }
        viewport = view
        for layer in [layers.ring, layers.cursor] {
            if let l = layer {
                picture = l.transformed(by: place).composited(over: picture)
                overlayLayers_.append(l.transformed(by: place))
            }
        }
        picture = picture.cropped(to: frameCI)
    }
    let rounded = picture.applyingFilter("CIBlendWithMask", parameters: [kCIInputBackgroundImageKey: clear, kCIInputMaskImageKey: mask])

    var result: CIImage
    if let backdrop = hit?.backdrop {
        result = backdrop
    } else {
        result = backgroundImage(style, take: take, project: project, geo: geo, canvas: canvas)
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
        backdropCache = Backdrop(key: key, mask: mask, backdrop: style.bgKind == "blur" ? nil : result)
    }
    result = rounded.composited(over: result).cropped(to: canvas)
    // P6 split: the camera part next to the card, nothing over the screen
    if let rects = splitRects, let camera {
        result = compositeSplitCamera(camera, over: result, rect: rects.camera, canvas: canvas)
    } else if let camera {
        // P4 camera overlay: after the card, so it never follows the zoom viewport
        result = compositeCamera(camera, over: result, style: style, geo: geo, canvas: canvas)
    }
    // P6 loupe (plan order: camera bubble -> loupe -> blur): the glass over everything, magnifying the screen
    if let l = fx.loupe {
        let cropPx = fx.camera.map { cropRect($0, take: CGSize(width: take.width, height: take.height)) }
        let takeW = cropPx?.width ?? CGFloat(take.width)
        let centre = CGPoint(x: frameCI.minX + CGFloat((l.cx - viewport.minX) / viewport.width) * frameCI.width,
                             y: frameCI.minY + (1 - CGFloat((l.cy - viewport.minY) / viewport.height)) * frameCI.height)
        result = compositeLoupe(l, over: result, take: take, centre: centre, displayPerTakePx: frameCI.width / takeW,
                                overlays: overlayLayers_, card: frameCI, cardRadius: geo.radius, canvas: canvas)
    }
    return (result, canvas)
}

// MARK: - Take access

final class ResultBox<T>: @unchecked Sendable { var value: Result<T, Error>? }

func blocking<T>(_ op: @escaping @Sendable () async throws -> T) throws -> T {
    let box = ResultBox<T>()
    let sem = DispatchSemaphore(value: 0)
    Task.detached {
        do { box.value = .success(try await op()) } catch { box.value = .failure(error) }
        sem.signal()
    }
    sem.wait()
    return try box.value!.get()
}

/// The movie of the last request, kept between `render.frame` calls: loading the asset and its
/// track properties took a good part of every preview frame (T26). Rebuilt when the file changes.
struct TakeReader {
    let url: URL
    let mtime: Double
    let generator: AVAssetImageGenerator
    let duration: Double
    let fps: Double
}

/// One entry per movie (the screen take and, P4, the camera take): alternating requests must not
/// rebuild the asset every time.
private var takeReaders: [String: TakeReader] = [:]

func reader(for url: URL) throws -> TakeReader {
    let stamp = mtime(url.path)
    if let r = takeReaders[url.path], r.url == url, r.mtime == stamp { return r }
    let asset = AVURLAsset(url: url)
    let (duration, fps) = try blocking {
        let d = try await asset.load(.duration).seconds
        let f = try await asset.loadTracks(withMediaType: .video).first?.load(.nominalFrameRate) ?? 30
        return (d, Double(f))
    }
    guard duration.isFinite, duration > 0 else { throw RenderError("take has no duration") }
    let gen = AVAssetImageGenerator(asset: asset)
    gen.appliesPreferredTrackTransform = true
    gen.requestedTimeToleranceBefore = .zero
    gen.requestedTimeToleranceAfter = .zero
    let r = TakeReader(url: url, mtime: stamp, generator: gen, duration: duration, fps: fps)
    takeReaders[url.path] = r
    return r
}

/// The take frame at exactly `t` (clamped into the movie): zero tolerance, so a scrubbed time
/// returns the frame covering it rather than a nearby keyframe.
///
/// A take written by SCStream starts at its first frame (~13 ms) while the track claims to start
/// at 0, and a request before that sample fails with "Cannot Open" (the editor opens at t = 0).
/// Within the first four frames a failed request is retried a little later until the first sample
/// is found.
func takeFrame(at url: URL, t: Double) throws -> CGImage {
    let r = try reader(for: url)
    let last = max(0, r.duration - 0.5 / max(r.fps, 1))
    let want = min(max(t, 0), last)
    let frame = 1 / max(r.fps, 1)
    var offset = 0.0
    while true {
        let at = CMTime(seconds: want + offset, preferredTimescale: 60000)
        switch generate(r.generator, at: at) {
        case .success(let image):
            return image
        case .failure(let e):
            if want + offset < 4 * frame, !(e is RenderError) {
                offset += frame / 8
                continue
            }
            takeReaders[url.path] = nil
            throw e
        }
    }
}

private func generate(_ gen: AVAssetImageGenerator, at: CMTime) -> Result<CGImage, Error> {
    let sem = DispatchSemaphore(value: 0)
    var image: CGImage?
    var failure: Error?
    gen.generateCGImagesAsynchronously(forTimes: [NSValue(time: at)]) { _, img, _, result, err in
        if result == .succeeded { image = img } else { failure = err ?? RenderError("frame unavailable") }
        sem.signal()
    }
    if sem.wait(timeout: .now() + 8) == .timedOut {
        gen.cancelAllCGImageGeneration()
        return .failure(RenderError("take frame timed out"))
    }
    guard let image else { return .failure(failure ?? RenderError("frame unavailable")) }
    return .success(image)
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
    /// Canonical text of the camera/cursor/ring/blur fields ("" = none).
    let fx: String
    /// `format:"raw"` frames are cached apart from the PNGs of the same key.
    let raw: Bool
    /// Camera take identity ("" = no camera overlay).
    let cameraKey: String
}

private struct CachedFrame {
    /// Encoded PNG, or the tightly packed RGBA8 pixels of a raw frame.
    let data: Data
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
    while frameCacheOrder.count > frameCacheLimit || (frameCacheOrder.count > 1 && frameCache.values.reduce(0) { $0 + $1.data.count } > frameCacheBytes) {
        frameCache[frameCacheOrder.removeFirst()] = nil
    }
}

func mtime(_ path: String) -> Double {
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

/// Raw frames reuse one file: overwritten in place (the consumer reads it before the next request,
/// so no half-written read is possible, and a rename per frame would only add a directory update).
private func writeInPlace(_ data: Data, to out: String) -> Bool {
    try? FileManager.default.createDirectory(atPath: (out as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
    let fd = open(out, O_WRONLY | O_CREAT | O_TRUNC, 0o644)
    guard fd >= 0 else { return false }
    defer { close(fd) }
    return data.withUnsafeBytes { p in
        var off = 0
        while off < p.count {
            let n = write(fd, p.baseAddress! + off, p.count - off)
            if n <= 0 { return false }
            off += n
        }
        return true
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

/// Reused pixel buffer of the raw hand-off (a 960 px frame is ~2 MB).
private var rawBuffer: [UInt8] = []

/// `render.frame`: {project, t, width, out?, format?, camera?, cursor?, ring?, blur?} -> {path, size, ms, cached}.
///
/// `format` is `"png"` (default, byte-identical to T24/T25) or `"raw"` (the editor preview): the file
/// holds the premultiplied RGBA8 pixels, `size[0] * 4` bytes per row, top row first, and the reply
/// adds `format:"raw"` and `stride`. The default `out` of a raw frame is the reused
/// `<project>/.cache/preview.rgba` (one consumer reads it before asking for the next frame). Raw
/// requests take the take frame through `takeFrameAuto` (sequential reader for consecutive times);
/// PNG requests keep the generator, so their bytes cannot change. The reply carries `decode`
/// (`reuse`/`advance`/`reopen`/`generator`) for diagnostics.
func handleRenderFrame(_ id: Any?, _ req: [String: Any]) {
    let started = DispatchTime.now()
    guard let project = req["project"] as? String, project.hasPrefix("/"),
          let t = num(req["t"]), t.isFinite, let w = num(req["width"]), w >= 16, w <= 8192 else {
        fail(id, "invalid", "render.frame needs an absolute project, a finite t and width 16...8192")
        return
    }
    let width = Int(w)
    guard let fx = FrameFx(request: req) else {
        fail(id, "invalid", "render.frame camera/cursor/ring/blur are malformed")
        return
    }
    let formatName = (req["format"] as? String) ?? "png"
    guard formatName == "png" || formatName == "raw" else {
        fail(id, "invalid", "render.frame format must be png or raw")
        return
    }
    let isRaw = formatName == "raw"
    let out = (req["out"] as? String) ?? (project as NSString).appendingPathComponent(
        isRaw ? ".cache/preview.rgba" : ".cache/f-\(Int(max(t, 0) * 1000))-\(width).png")
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

    let cameraSpec = CameraSpec(json: json, project: project)
    let key = FrameKey(project: project, mtime: mtime(projectFile), takeMtime: mtime(takePath),
                       t: Int64((max(t, 0) * 1_000_000).rounded()), width: width, fx: fx.key, raw: isRaw,
                       cameraKey: cameraSpec?.cacheKey ?? "")
    let ms = { Double(DispatchTime.now().uptimeNanoseconds - started.uptimeNanoseconds) / 1_000_000 }
    let rounded = { (v: Double) in (v * 10).rounded() / 10 }
    func done(_ size: [Int], cached: Bool, decode: String) {
        var body: [String: Any] = ["path": out, "size": size, "ms": rounded(ms()), "cached": cached, "decode": decode]
        if isRaw { body["format"] = "raw"; body["stride"] = size[0] * 4 }
        reply(id, body)
    }
    if let hit = frameCache[key] {
        guard writeAtomically(hit.data, to: out) else { fail(id, "internal", "could not write \(out)"); return }
        done(hit.size, cached: true, decode: "cache")
        return
    }
    do {
        let style = RenderStyle(json: json)
        let url = URL(fileURLWithPath: takePath)
        let size: [Int]
        let data: Data
        var decode = "generator"
        if isRaw {
            let (take, served) = try takeFrameAuto(at: url, t: t, mode: (req["decode"] as? String) == "generator" ? .generator : .auto)
            decode = served
            let cam = cameraSpec.flatMap { cameraDraw($0, at: t, sequential: (req["decode"] as? String) != "generator", fx: fx) }
            guard let dims = composeStyledRaw(take: take, style: style, project: project, width: width, fx: fx, camera: cam, into: &rawBuffer) else {
                fail(id, "internal", "could not compose the frame")
                return
            }
            size = [dims.width, dims.height]
            data = Data(rawBuffer)
            guard writeInPlace(data, to: out) else { fail(id, "internal", "could not write \(out)"); return }
        } else {
            let take = TakeFrame(cg: try takeFrame(at: url, t: t))
            let cam = cameraSpec.flatMap { cameraDraw($0, at: t, sequential: false, fx: fx) }
            guard let image = composeStyledFrame(take: take, style: style, project: project, width: width, fx: fx, camera: cam),
                  let png = encodePNG(image) else {
                fail(id, "internal", "could not compose the frame")
                return
            }
            guard writeAtomically(png, to: out) else { fail(id, "internal", "could not write \(out)"); return }
            size = [image.width, image.height]
            data = png
        }
        cacheStore(key, CachedFrame(data: data, size: size))
        pruneFrameCache(project: project, keep: out)
        done(size, cached: false, decode: decode)
    } catch let e as RenderError {
        fail(id, "internal", e.message)
    } catch {
        fail(id, "internal", "render.frame failed: \(error.localizedDescription)")
    }
}
