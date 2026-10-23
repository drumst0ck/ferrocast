import CoreGraphics
import CoreImage
import Foundation

// Cursor sprites, click ring and directional blur for `render.frame` (T25). Everything is vector
// paths drawn in code (no bitmap assets). Sprite space is y-down with the hot spot at the origin
// and a nominal height of about 24 units; the caller scales by `cursor.size` and the output width.

/// Sprite outlines in sprite space, y down. `hand` and `arrow` have the hot spot at the tip; the
/// other shapes are centred on it.
private func spritePath(_ kind: String) -> (fill: CGPath, line: CGPath?, filled: Bool, white: Bool) {
    switch kind {
    case "ibeam":
        let p = CGMutablePath()
        p.move(to: CGPoint(x: 0, y: -9)); p.addLine(to: CGPoint(x: 0, y: 9))
        p.move(to: CGPoint(x: -3.2, y: -9)); p.addLine(to: CGPoint(x: 3.2, y: -9))
        p.move(to: CGPoint(x: -3.2, y: 9)); p.addLine(to: CGPoint(x: 3.2, y: 9))
        return (p, nil, false, false)
    case "crosshair":
        let p = CGMutablePath()
        for (a, b) in [((-9.0, 0.0), (-2.5, 0.0)), ((2.5, 0.0), (9.0, 0.0)), ((0.0, -9.0), (0.0, -2.5)), ((0.0, 2.5), (0.0, 9.0))] {
            p.move(to: CGPoint(x: a.0, y: a.1)); p.addLine(to: CGPoint(x: b.0, y: b.1))
        }
        p.addEllipse(in: CGRect(x: -1, y: -1, width: 2, height: 2))
        return (p, nil, false, false)
    case "hand", "grab":
        let p = CGMutablePath()
        if kind == "hand" {
            p.addRoundedRect(in: CGRect(x: -2.2, y: 0, width: 4.4, height: 13), cornerWidth: 2.2, cornerHeight: 2.2)
            p.addRoundedRect(in: CGRect(x: -7.5, y: 9, width: 15, height: 11), cornerWidth: 4.5, cornerHeight: 4.5)
        } else {
            p.addRoundedRect(in: CGRect(x: -8, y: -4, width: 16, height: 11), cornerWidth: 4.5, cornerHeight: 4.5)
            for i in 0..<4 {
                p.addRoundedRect(in: CGRect(x: -7.4 + Double(i) * 3.9, y: -8, width: 3.4, height: 7), cornerWidth: 1.7, cornerHeight: 1.7)
            }
        }
        return (p, nil, true, true)
    case "resize-h", "resize-v":
        let p = CGMutablePath()
        p.move(to: CGPoint(x: -7, y: 0)); p.addLine(to: CGPoint(x: 7, y: 0))
        for s in [-1.0, 1.0] {
            p.move(to: CGPoint(x: s * 9.5, y: 0)); p.addLine(to: CGPoint(x: s * 5.2, y: -3.6))
            p.addLine(to: CGPoint(x: s * 5.2, y: 3.6)); p.closeSubpath()
        }
        if kind == "resize-v" {
            var rot = CGAffineTransform(rotationAngle: .pi / 2)
            return (p.copy(using: &rot) ?? p, nil, true, false)
        }
        return (p, nil, true, false)
    default: // arrow
        let p = CGMutablePath()
        p.move(to: .zero)
        for (x, y) in [(0.0, 17.0), (4.2, 13.2), (7.2, 20.0), (10.0, 18.8), (7.1, 12.2), (12.5, 12.2)] {
            p.addLine(to: CGPoint(x: x, y: y))
        }
        p.closeSubpath()
        return (p, nil, true, false)
    }
}

/// Draws one cursor sprite with its hot spot at `at` (layer pixels, top-left origin). `scale`
/// converts sprite units to pixels. Outline and fill are grouped so the shadow is drawn once.
func drawCursorSprite(_ ctx: CGContext, kind: String, at: CGPoint, scale: CGFloat, opacity: CGFloat) {
    let (path, _, filled, whiteFill) = spritePath(kind)
    ctx.saveGState()
    ctx.translateBy(x: at.x, y: at.y)
    ctx.scaleBy(x: scale, y: scale)
    ctx.setAlpha(opacity)
    // Shadow offsets live in base space (y up): the shadow falls below and slightly right.
    ctx.setShadow(offset: CGSize(width: 0.5 * scale, height: -1.2 * scale), blur: 2.2 * scale,
                  color: CGColor(red: 0, green: 0, blue: 0, alpha: 0.45))
    ctx.beginTransparencyLayer(auxiliaryInfo: nil)
    ctx.setLineJoin(.round)
    ctx.setLineCap(.round)
    let black = CGColor(red: 0, green: 0, blue: 0, alpha: 1), white = CGColor(red: 1, green: 1, blue: 1, alpha: 1)
    if filled {
        ctx.addPath(path)
        ctx.setFillColor(whiteFill ? white : black)
        ctx.setStrokeColor(whiteFill ? black : white)
        ctx.setLineWidth(1.6)
        ctx.drawPath(using: .fillStroke)
    } else {
        // thin glyphs: white halo under a black stroke so they read on any background
        ctx.addPath(path); ctx.setStrokeColor(white); ctx.setLineWidth(3.6); ctx.strokePath()
        ctx.addPath(path); ctx.setStrokeColor(black); ctx.setLineWidth(1.5); ctx.strokePath()
    }
    ctx.endTransparencyLayer()
    ctx.restoreGState()
}

/// Click ring: a faint fill, a dark halo and a white stroke whose alpha follows the envelope.
func drawClickRing(_ ctx: CGContext, center: CGPoint, radius: CGFloat, alpha: CGFloat, line: CGFloat) {
    guard radius > 0.5, alpha > 0 else { return }
    let rect = CGRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2)
    ctx.saveGState()
    ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.14 * alpha))
    ctx.fillEllipse(in: rect)
    ctx.setStrokeColor(CGColor(red: 0, green: 0, blue: 0, alpha: 0.35 * alpha))
    ctx.setLineWidth(line + 2)
    ctx.strokeEllipse(in: rect)
    ctx.setStrokeColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.95 * alpha))
    ctx.setLineWidth(line)
    ctx.strokeEllipse(in: rect)
    ctx.restoreGState()
}

/// A transparent layer of `size` pixels drawn with CoreGraphics in top-left coordinates.
func drawnLayer(size: CGSize, _ draw: (CGContext) -> Void) -> CIImage? {
    guard let ctx = CGContext(data: nil, width: Int(size.width), height: Int(size.height), bitsPerComponent: 8, bytesPerRow: 0,
                              space: renderSRGB, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    ctx.translateBy(x: 0, y: size.height)
    ctx.scaleBy(x: 1, y: -1)
    draw(ctx)
    guard let img = ctx.makeImage() else { return nil }
    return CIImage(cgImage: img, options: [.colorSpace: NSNull()])
}

/// Deterministic directional blur by accumulation: `samples` copies shifted evenly along `dir`
/// (y down) over `length` pixels, each weighted 1/samples. `samples` is a minimum: a long streak
/// gets about one tap per 1.5 px (at most 32) so it reads as a smear, not as separate ghosts.
/// The extent is that of `img`.
func motionBlurred(_ img: CIImage, dir: [Double], length: Double, samples: Int, clamped: Bool) -> CIImage {
    guard samples > 1, length > 0.25, dir.count == 2 else { return img }
    let n = min(32, max(samples, Int((length / 1.5).rounded(.up)) + 1))
    let extent = img.extent
    let base = clamped ? img.clampedToExtent() : img
    var acc: CIImage?
    let w = CGFloat(1) / CGFloat(n)
    for k in 0..<n {
        let f = Double(k) / Double(n - 1) - 0.5
        let shifted = base.transformed(by: CGAffineTransform(translationX: CGFloat(dir[0] * length * f), y: CGFloat(-dir[1] * length * f)))
        let weighted = shifted.applyingFilter("CIColorMatrix", parameters: [
            "inputRVector": CIVector(x: w, y: 0, z: 0, w: 0), "inputGVector": CIVector(x: 0, y: w, z: 0, w: 0),
            "inputBVector": CIVector(x: 0, y: 0, z: w, w: 0), "inputAVector": CIVector(x: 0, y: 0, z: 0, w: w),
        ])
        acc = acc.map { weighted.applyingFilter("CIAdditionCompositing", parameters: [kCIInputBackgroundImageKey: $0]) } ?? weighted
    }
    return (acc ?? img).cropped(to: extent)
}
