import AVFoundation
import CoreGraphics
import CoreImage
import CoreVideo
import Foundation

// Sequential take decoding for playback and export (S15, S16; measured on a real Retina take).
//
// `AVAssetImageGenerator` opens a decode session per request: ~27 ms for a 2940x1912 HEVC frame, the
// biggest single cost of a preview frame. Playback and export ask for CONSECUTIVE frames, which an
// `AVAssetReader` + `AVAssetReaderTrackOutput` serves from one running decoder. Random access (a scrub,
// a seek, the first frame) keeps using the generator in Render.swift: reopening a reader costs a decode
// from the previous keyframe, which only pays off when the next requests follow.
//
// The decoder holds the sample that covers the requested time (`cur`) and the NEXT sample (`next`,
// always read one ahead after a request, so "does the next sample start at or before t?" is a plain
// comparison). A request is served by one of three steps, decided by the pure `decoderStep`:
//   reuse   cur.pts <= t < next.pts (or the reader is at its end): same pixel buffer, no decode
//           (a repeated time, or a time between two samples of a variable-rate take);
//   advance t is ahead of `next` by at most `maxAheadSeconds`: read and drop samples until the one
//           covering t (export at 2x speed, a playback frame that was skipped);
//   reopen  anything else (first request, a jump back, a jump far ahead, a failed reader).

/// What `SeqDecoder` does for a request. Pure so it can be checked without a movie.
enum DecoderStep: Equatable { case reuse, advance, reopen }

/// A forward jump longer than this is not worth decoding through: measured on a 2940x1912 HEVC take,
/// reading and dropping a frame costs ~2 ms while a generator request costs ~40 ms, so the break-even
/// is ~0.25 s (15 frames: 0.25 s steps ran at 42 ms/frame either way, 0.9 s steps at 160 ms/frame
/// when decoded through). 0.2 keeps a margin. Playback steps are 17-33 ms and export steps at up to
/// 8x speed are below it; faster speeds simply use the generator for every frame, as before.
let maxAheadSeconds = 0.2

/// `cur`/`next` are the presentation times of the held and the following sample (`next` nil at the
/// end of the movie); `want` is the requested time. Backward requests and a missing `cur` reopen.
func decoderStep(cur: CMTime?, next: CMTime?, want: CMTime, limit: Double = maxAheadSeconds) -> DecoderStep {
    guard let cur, want >= cur else { return .reopen }
    guard let next else { return .reuse }
    if want < next { return .reuse }
    return (want - cur).seconds <= limit ? .advance : .reopen
}

/// A take frame ready for composition, from either decoder. `image` is colour managed (the take's
/// own colour space to the working space) exactly as `CIImage(cgImage:)` of the generator's image;
/// `raw` is the untagged variant used by the `blur` background.
struct TakeFrame {
    let width: Int
    let height: Int
    let image: CIImage
    let raw: () -> CIImage

    init(cg: CGImage) {
        width = cg.width
        height = cg.height
        image = CIImage(cgImage: cg)
        raw = { CIImage(cgImage: cg, options: [.colorSpace: NSNull()]) }
    }

    init(buffer: CVPixelBuffer) {
        width = CVPixelBufferGetWidth(buffer)
        height = CVPixelBufferGetHeight(buffer)
        image = CIImage(cvPixelBuffer: buffer)
        raw = { CIImage(cvPixelBuffer: buffer, options: [.colorSpace: NSNull()]) }
    }
}

/// Pixel format the reader converts to. BGRA keeps the colour conversion in the decoder's own pixel
/// transfer (hardware) and gives CoreImage a plain RGB texture; 420v would move it to the compose
/// graph. Measured on the real take (see the commit notes); BGRA is also the closest to what the
/// generator returned.
private let readerPixelFormat = kCVPixelFormatType_32BGRA

final class SeqDecoder {
    let url: URL
    let stamp: Double
    private let asset: AVURLAsset
    private let track: AVAssetTrack
    private let duration: CMTime
    /// Nominal frame time (used to start a reopened reader a little before the request).
    private let frameTime: Double
    private var reader: AVAssetReader?
    private var output: AVAssetReaderTrackOutput?
    private var cur: (pts: CMTime, sample: CMSampleBuffer)?
    private var next: (pts: CMTime, sample: CMSampleBuffer)?
    private var atEnd = false
    /// How the last `frame(at:)` was served, for the smoke checks and the timing logs.
    private(set) var lastStep: DecoderStep = .reopen
    private(set) var opens = 0
    private(set) var decoded = 0

    /// nil when the movie cannot be read sequentially (no video track, a rotated track: the generator
    /// applies the preferred transform, the reader does not).
    init?(url: URL) {
        self.url = url
        stamp = mtime(url.path)
        let asset = AVURLAsset(url: url)
        self.asset = asset
        guard let loaded: (AVAssetTrack, CMTime, Float) = try? blocking({
            guard let t = try await asset.loadTracks(withMediaType: .video).first else { throw RenderError("no video track") }
            let (d, f, tr) = try await (asset.load(.duration), t.load(.nominalFrameRate), t.load(.preferredTransform))
            guard tr.isIdentity else { throw RenderError("rotated track") }
            return (t, d, f)
        }) else { return nil }
        track = loaded.0
        duration = loaded.1
        frameTime = 1 / Double(max(loaded.2, 1))
    }

    /// What a request for `want` would do now (nil when no reader is open).
    func wouldStep(_ want: CMTime, limit: Double = maxAheadSeconds) -> DecoderStep? {
        reader == nil ? nil : decoderStep(cur: cur?.pts, next: next?.pts, want: want, limit: limit)
    }

    /// The frame covering `t` (a time already clamped into the movie), or nil when the reader fails
    /// (the caller falls back to the generator).
    func frame(at want: CMTime, limit: Double = maxAheadSeconds) -> CVPixelBuffer? {
        var step = decoderStep(cur: cur?.pts, next: next?.pts, want: want, limit: limit)
        if step != .reopen, reader?.status == .failed { step = .reopen }
        if step == .reopen, !open(at: want) { return nil }
        if step == .advance { advance(to: want) }
        lastStep = step
        guard let cur, let buffer = CMSampleBufferGetImageBuffer(cur.sample) else { return nil }
        return buffer
    }

    /// Start a reader at (a little before) `want` and read until `cur` covers it.
    /// A take written by SCStream sends a frame only when the screen changes, so the sample covering
    /// `want` can be far before it: when the first samples are all later than `want` the range start
    /// is moved back, up to the beginning of the movie.
    private func open(at want: CMTime) -> Bool {
        cur = nil
        next = nil
        atEnd = false
        for back in [3 * frameTime, 1.0, 8.0, .infinity] {
            let start = back == .infinity ? CMTime.zero : max(.zero, want - CMTime(seconds: back, preferredTimescale: 60000))
            guard let r = try? AVAssetReader(asset: asset) else { return false }
            let out = AVAssetReaderTrackOutput(track: track, outputSettings: [
                kCVPixelBufferPixelFormatTypeKey as String: readerPixelFormat,
                kCVPixelBufferIOSurfacePropertiesKey as String: [String: Any](),
                kCVPixelBufferMetalCompatibilityKey as String: true,
            ])
            out.alwaysCopiesSampleData = false
            r.timeRange = CMTimeRange(start: start, end: duration)
            guard r.canAdd(out) else { return false }
            r.add(out)
            guard r.startReading() else { return false }
            reader = r
            output = out
            opens += 1
            guard let first = readSample() else { continue }
            cur = first
            next = nil
            // the first sample may already be later than `want` at the very start of a take (the
            // generator path accepts that too): that is the frame to show
            if first.pts > want, start > .zero { cur = nil; continue }
            advance(to: want)
            return true
        }
        return false
    }

    private func readSample() -> (pts: CMTime, sample: CMSampleBuffer)? {
        while let s = output?.copyNextSampleBuffer() {
            decoded += 1
            // an empty/marker buffer carries no image
            guard CMSampleBufferGetImageBuffer(s) != nil else { continue }
            return (CMSampleBufferGetPresentationTimeStamp(s), s)
        }
        return nil
    }

    /// Make `cur` the last sample with pts <= `want` and `next` the one after it.
    private func advance(to want: CMTime) {
        if next == nil, !atEnd {
            next = readSample()
            atEnd = next == nil
        }
        while let n = next, n.pts <= want {
            cur = n
            next = readSample()
            atEnd = next == nil
        }
    }

    /// Release the reader (an idle decoder should not keep a hardware session).
    func close() {
        reader?.cancelReading()
        reader = nil
        output = nil
        cur = nil
        next = nil
    }
}

/// The sequential reader and the time of the previous request of ONE track. The screen take and the
/// camera take (P4) each have their own: one reader per track, reopened on seeks.
final class DecodeState {
    fileprivate var seqDecoder: SeqDecoder?
    /// Time of the previous request, to recognise consecutive requests (playback) from random ones (scrubbing).
    fileprivate var lastWant: CMTime?

    func close() {
        seqDecoder?.close()
        seqDecoder = nil
        lastWant = nil
    }
}

private let takeState = DecodeState()
/// Decoder state of `camera.mov` (see `CameraOverlay.swift`).
let cameraState = DecodeState()

/// Which decoder a request may use. `generator` is the old per-frame path, kept as the reference for
/// the smoke checks and as a fallback knob (`"decode":"generator"` in a `render.frame` or `render` request).
enum TakeDecode {
    /// Sequential for consecutive requests, generator for random access (preview and export).
    case auto
    case generator
}

/// The take frame at `t` for playback and export. `served` says which path answered:
/// `reuse`/`advance`/`reopen` (sequential reader), `generator` (random access).
///
/// Policy (measured, see `maxAheadSeconds`): a request is served sequentially when the reader is open
/// and the step is not a reopen, or when it follows the previous request closely forward (the
/// second frame of a playback run or of an export opens the reader). Anything else, a first frame, a
/// seek, a scrub jump, a step back, a trim or speed cut, goes to the generator, which is cheaper than
/// a reopen for a single frame; the frame after it opens the reader again. A reader that fails falls
/// back to the generator.
func takeFrameAuto(at url: URL, t: Double, mode: TakeDecode = .auto, state: DecodeState = takeState) throws -> (frame: TakeFrame, served: String) {
    let r = try reader(for: url)
    let last = max(0, r.duration - 0.5 / max(r.fps, 1))
    let want = CMTime(seconds: min(max(t, 0), last), preferredTimescale: 60000)
    defer { state.lastWant = want }
    let stamp = mtime(url.path)
    if let d = state.seqDecoder, d.url != url || d.stamp != stamp {
        d.close()
        state.seqDecoder = nil
    }
    let follows = state.lastWant.map { want > $0 && (want - $0).seconds <= maxAheadSeconds } ?? false
    // a request that follows the previous one closely may be up to a frame or two further from the
    // held sample than the limit (the previous request was served by `reuse` or by the generator):
    // twice the limit there keeps a steady 0.2 s step from flipping between advance and reopen
    let limit = follows ? 2 * maxAheadSeconds : maxAheadSeconds
    let open = state.seqDecoder?.wouldStep(want, limit: limit).map { $0 != .reopen } ?? false
    if mode == .auto, open || follows {
        if state.seqDecoder == nil { state.seqDecoder = SeqDecoder(url: url) }
        if let d = state.seqDecoder, let buffer = d.frame(at: want, limit: limit) {
            return (TakeFrame(buffer: buffer), "\(d.lastStep)")
        }
        state.seqDecoder?.close()
        state.seqDecoder = nil
    }
    return (TakeFrame(cg: try takeFrame(at: url, t: t)), "generator")
}

/// Drop the sequential decoders (an export ended, the project changed).
func closeSequentialDecoder() {
    takeState.close()
    cameraState.close()
}

/// Diagnostics for the smoke checks and the timing logs: (reader opens, samples decoded) of the screen take.
func sequentialDecoderStats() -> (opens: Int, decoded: Int) { (takeState.seqDecoder?.opens ?? 0, takeState.seqDecoder?.decoded ?? 0) }

/// Same for the camera take.
func cameraDecoderStats() -> (opens: Int, decoded: Int) { (cameraState.seqDecoder?.opens ?? 0, cameraState.seqDecoder?.decoded ?? 0) }
