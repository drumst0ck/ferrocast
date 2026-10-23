import AVFoundation
import CoreGraphics
import CoreImage
import CoreVideo
import Foundation
import ImageIO
import UniformTypeIdentifiers

// `render`: export a project to an MP4 or an animated GIF (T28 + T29, S16).
//
// Request: {id, cmd:"render", project, out, format:"mp4"|"gif", codec:"h264"|"hevc", width, height, fps,
// quality:"standard"|"high", frames, segments:[{src_start,src_end,speed}], params_file}. Paths travel
// in JSON only. Events (all carry the request id): `started {total_frames}` once the writer is open,
// `progress {done_frames,total_frames,eta}` about 4 times a second, `heartbeat` when nothing else was
// sent for 10 s (the client treats 45 s of silence as a hang). The final line is `ok {path, bytes,
// duration, frames}` or a structured error (`cancelled` after a `cancel`).
//
// Single source of truth: Rust (`capture::export`) decides the frame count, the size and, for every
// output frame, the SOURCE time and the camera/cursor/ring/blur parameters; they arrive as one NDJSON
// line per frame in `params_file` (a file, not the request, so 60 fps x minutes never bloats a pipe
// line, and it is read in flat memory). Swift only decodes the take frame, composes it with the SAME
// code as `render.frame` (`composeStyledImage` in Render.swift) and encodes. `segments` are not needed
// to place frames; they are cross-checked against `frames` and kept for the audio mix of T33.
//
// GIF (`format:"gif"`, T29): the SAME frames, params file, progress/heartbeat/cancel and `.partial`
// rules; the frames go to an ImageIO `CGImageDestination` (`UTType.gif`) with a per-frame delay and
// loop count 0 (forever). Limits (S16): width <= 960, fps <= 15, frames x pixels <= 300 x 960 x 540 (the helper re-checks;
// `export_plan` clamps before asking). `codec` and `quality` are ignored. Delays are centiseconds:
// frame i ends at round((i+1)*100/fps) and lasts the difference to the previous end, so the rounding
// is spread and the total is round(frames*100/fps) (see `gifDelays`, mirrored by `export::gif_delays`).
// ImageIO keeps every frame until `CGImageDestinationFinalize` (no streaming API), hence the budget
// (measured: 150 frames 960x540 = 897 MB peak, 450 frames = 2.2 GB);
// the quantisation (palette, dither) is ImageIO's. Finalize runs on its own thread so heartbeats and
// `cancel` keep being served (a cancel during finalize waits for it, then removes the file).
//
// Take decoding: sequential (`AVAssetReader`, Decode.swift) while the source times of consecutive output
// frames are close, one generator request for the first frame and after every trim or speed cut. A
// request field `"decode":"generator"` forces the old one-request-per-frame path (reference for the
// smoke check; not sent by the app). 1080p/30 of a real Retina take: 18 -> 75 fps.
//
// Audio: OUT OF SCOPE until T33. The writer is built so that an audio input can be added next to the
// video input before `startWriting` (mapping `screen.mov`'s audio through `segments`); today the MP4
// has no audio track.
//
// Output: `<out>.partial` is written and renamed to `<out>` only on success; failure and cancel remove it.
// Threading: the export runs on its own thread so that stdin (`cancel`) keeps being served. Only one
// export at a time (`busy`); `render.frame` answers `busy` meanwhile because both share the take reader.

private let exportLock = NSLock()
private var activeExport: ExportJob?

final class ExportJob {
    let id: Any?
    let numericId: Int
    let finished = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var cancelRequested = false

    init(id: Any?) {
        self.id = id
        numericId = (id as? NSNumber)?.intValue ?? -1
    }

    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelRequested
    }

    func cancel() {
        lock.lock()
        cancelRequested = true
        lock.unlock()
    }
}

struct ExportSpec {
    var project: String
    var out: String
    var format: String
    var codec: String
    var width: Int
    var height: Int
    var fps: Int
    var quality: String
    var frames: Int
    var paramsFile: String
    var segmentSeconds: Double
    /// `"decode":"generator"` in the request: the old one-request-per-frame path (reference for the smoke check).
    var generatorOnly = false
}

/// Same limits as `capture::export` in Rust (the helper re-checks what it is sent).
private let maxLongEdge = 3840
private let maxPixels = 3840 * 2160
/// GIF limits (S16 + T29 measurement), mirrored in `capture::export` (`GIF_MAX_*`).
private let gifMaxWidth = 960
private let gifMaxFps = 15
/// Memory budget in frames x pixels (ImageIO keeps every frame until finalize); see `GIF_MAX_PIXEL_FRAMES`.
private let gifMaxPixelFrames = 300 * 960 * 540

func exportInProgress() -> Bool {
    exportLock.lock()
    defer { exportLock.unlock() }
    return activeExport != nil
}

func handleRender(_ id: Any?, _ req: [String: Any]) {
    func int(_ k: String) -> Int? {
        guard let n = req[k] as? NSNumber, n.doubleValue.rounded() == n.doubleValue, abs(n.doubleValue) < 1e9 else { return nil }
        return n.intValue
    }
    guard let project = req["project"] as? String, project.hasPrefix("/"),
          let out = req["out"] as? String, out.hasPrefix("/"), !out.hasSuffix("/"),
          let paramsFile = req["params_file"] as? String, paramsFile.hasPrefix("/") else {
        fail(id, "invalid", "render needs absolute project, out and params_file paths")
        return
    }
    guard let format = req["format"] as? String, format == "mp4" || format == "gif" else {
        fail(id, "invalid", "render format must be mp4 or gif")
        return
    }
    let codec = (req["codec"] as? String) ?? "h264"
    guard format == "gif" || codec == "h264" || codec == "hevc" else {
        fail(id, "invalid", "render codec must be h264 or hevc")
        return
    }
    let quality = (req["quality"] as? String) ?? "standard"
    guard quality == "standard" || quality == "high" else {
        fail(id, "invalid", "render quality must be standard or high")
        return
    }
    guard let w = int("width"), let h = int("height"), w >= 16, h >= 16, w % 2 == 0, h % 2 == 0,
          w <= maxLongEdge, h <= maxLongEdge, w * h <= maxPixels else {
        fail(id, "invalid", "render size must be even, at least 16 px and at most 3840x2160 pixels")
        return
    }
    guard let fps = int("fps"), fps >= 1, fps <= 60 else {
        fail(id, "invalid", "render fps must be 1...60")
        return
    }
    guard let frames = int("frames"), frames >= 1, frames <= 60 * 3600 * 6 else {
        fail(id, "invalid", "render frames must be at least 1")
        return
    }
    if format == "gif" {
        guard w <= gifMaxWidth else {
            fail(id, "invalid", "a GIF can be at most \(gifMaxWidth) px wide")
            return
        }
        guard fps <= gifMaxFps else {
            fail(id, "invalid", "a GIF can be at most \(gifMaxFps) fps")
            return
        }
        guard frames * w * h <= gifMaxPixelFrames else {
            fail(id, "invalid", "this GIF is too long: at most \(gifMaxPixelFrames / (w * h)) frames at \(w)x\(h) (trim it or export an MP4)")
            return
        }
    }
    guard let segs = req["segments"] as? [[String: Any]], !segs.isEmpty else {
        fail(id, "invalid", "render needs at least one segment")
        return
    }
    var seconds = 0.0
    for s in segs {
        guard let a = num(s["src_start"]), let b = num(s["src_end"]), let k = num(s["speed"]),
              a.isFinite, b.isFinite, k.isFinite, b > a, k > 0 else {
            fail(id, "invalid", "render segments are malformed")
            return
        }
        seconds += (b - a) / k
    }
    // The frame count and the segments come from the same Rust plan: they agree within one frame.
    guard abs(seconds - Double(frames) / Double(fps)) <= 1.0 / Double(fps) + 1e-6 else {
        fail(id, "invalid", "render segments last \(seconds) s but \(frames) frames at \(fps) fps do not match")
        return
    }
    guard FileManager.default.fileExists(atPath: paramsFile) else {
        fail(id, "not-found", "params_file not found")
        return
    }
    let job = ExportJob(id: id)
    exportLock.lock()
    if activeExport != nil {
        exportLock.unlock()
        fail(id, "busy", "an export is already running")
        return
    }
    activeExport = job
    exportLock.unlock()
    let spec = ExportSpec(project: project, out: out, format: format, codec: codec, width: w, height: h, fps: fps, quality: quality,
                          frames: frames, paramsFile: paramsFile, segmentSeconds: seconds,
                          generatorOnly: (req["decode"] as? String) == "generator")
    Thread.detachNewThread {
        if spec.format == "gif" { runGifExport(job, spec) } else { runExport(job, spec) }
        closeSequentialDecoder()
        releaseExportSlot(job)
        job.finished.signal()
    }
}

/// Free the single export slot. Called BEFORE the final `ok`/error line is written, so a client that
/// starts the next request the moment it reads that line never meets `busy`. Idempotent.
private func releaseExportSlot(_ job: ExportJob) {
    exportLock.lock()
    if activeExport === job { activeExport = nil }
    exportLock.unlock()
}

func handleCancel(_ id: Any?, _ req: [String: Any]) {
    let target = (req["target"] as? NSNumber)?.intValue
    exportLock.lock()
    let job = activeExport
    exportLock.unlock()
    guard let job, job.numericId == target else {
        fail(id, "not-found", "no running request with that id")
        return
    }
    job.cancel()
    reply(id, [:])
}

/// Stdin closed or SIGTERM: stop the export and wait (bounded) for its thread to remove the partial.
func shutdownExport() {
    exportLock.lock()
    let job = activeExport
    exportLock.unlock()
    guard let job else { return }
    job.cancel()
    _ = job.finished.wait(timeout: .now() + 10)
}

/// Lines of a file without loading it (1 MiB chunks).
private final class LineReader {
    private let handle: FileHandle
    private var buffer = Data()
    private var eof = false

    init?(path: String) {
        guard let h = FileHandle(forReadingAtPath: path) else { return nil }
        handle = h
    }

    deinit { try? handle.close() }

    func next() -> Data? {
        while true {
            if let nl = buffer.firstIndex(of: 0x0A) {
                let line = buffer[buffer.startIndex..<nl]
                buffer = Data(buffer[buffer.index(after: nl)...])
                if line.allSatisfy({ $0 == 0x20 || $0 == 0x0D }) { continue }
                return Data(line)
            }
            if eof {
                if buffer.allSatisfy({ $0 == 0x20 || $0 == 0x0D }) { return nil }
                defer { buffer = Data() }
                return buffer
            }
            let chunk = (try? handle.read(upToCount: 1 << 20)) ?? Data()
            if chunk.isEmpty { eof = true } else { buffer.append(chunk) }
        }
    }
}

private func isDiskFull(_ error: Error?) -> Bool {
    guard let e = error as NSError? else { return false }
    if e.domain == NSPOSIXErrorDomain && e.code == 28 { return true }
    if e.domain == NSCocoaErrorDomain && e.code == NSFileWriteOutOfSpaceError { return true }
    if e.domain == AVFoundationErrorDomain && e.code == AVError.diskFull.rawValue { return true }
    if let u = e.userInfo[NSUnderlyingErrorKey] as? NSError, u !== e { return isDiskFull(u) }
    return false
}

private func bitrate(_ spec: ExportSpec) -> Int {
    // Bits per pixel per frame: screen content compresses far better than camera footage.
    var bpp = spec.codec == "hevc" ? 0.05 : 0.08
    if spec.quality == "high" { bpp *= 1.6 }
    return max(1_000_000, Int(Double(spec.width * spec.height) * Double(spec.fps) * bpp))
}

private func runExport(_ job: ExportJob, _ spec: ExportSpec) {
    let id = job.id
    let fm = FileManager.default
    let partial = spec.out + ".partial"
    var writer: AVAssetWriter?
    func abort(_ code: String, _ message: String) {
        writer?.cancelWriting()
        try? fm.removeItem(atPath: partial)
        releaseExportSlot(job)
        fail(id, code, message)
    }

    let projectFile = (spec.project as NSString).appendingPathComponent("project.json")
    guard let raw = fm.contents(atPath: projectFile) else { return abort("not-found", "project.json not found") }
    guard let json = (try? JSONSerialization.jsonObject(with: raw)) as? [String: Any] else {
        return abort("invalid", "project.json is not valid")
    }
    let file = ((json["sources"] as? [String: Any])?["screen"] as? [String: Any])?["file"] as? String ?? "screen.mov"
    guard !file.isEmpty, !file.contains("/"), !file.contains("..") else {
        return abort("invalid", "sources.screen.file must be a plain file name")
    }
    let takePath = (spec.project as NSString).appendingPathComponent(file)
    guard fm.fileExists(atPath: takePath) else { return abort("not-found", "\(file) not found") }
    guard let lines = LineReader(path: spec.paramsFile) else { return abort("not-found", "params_file could not be opened") }
    let style = RenderStyle(json: json)
    let takeURL = URL(fileURLWithPath: takePath)

    try? fm.createDirectory(atPath: (spec.out as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
    try? fm.removeItem(atPath: partial)

    var compression: [String: Any] = [
        AVVideoAverageBitRateKey: bitrate(spec),
        AVVideoExpectedSourceFrameRateKey: spec.fps,
        AVVideoMaxKeyFrameIntervalKey: spec.fps * 2,
    ]
    if spec.codec == "h264" { compression[AVVideoProfileLevelKey] = AVVideoProfileLevelH264HighAutoLevel }
    let settings: [String: Any] = [
        AVVideoCodecKey: spec.codec == "hevc" ? AVVideoCodecType.hevc : AVVideoCodecType.h264,
        AVVideoWidthKey: spec.width,
        AVVideoHeightKey: spec.height,
        AVVideoCompressionPropertiesKey: compression,
        AVVideoColorPropertiesKey: [
            AVVideoColorPrimariesKey: AVVideoColorPrimaries_ITU_R_709_2,
            AVVideoTransferFunctionKey: AVVideoTransferFunction_ITU_R_709_2,
            AVVideoYCbCrMatrixKey: AVVideoYCbCrMatrix_ITU_R_709_2,
        ],
    ]
    let w: AVAssetWriter
    do { w = try AVAssetWriter(outputURL: URL(fileURLWithPath: partial), fileType: .mp4) } catch {
        return abort("internal", "could not create \(partial): \(error.localizedDescription)")
    }
    writer = w
    guard w.canApply(outputSettings: settings, forMediaType: .video) else {
        return abort("invalid", "this Mac cannot encode \(spec.codec) at \(spec.width)x\(spec.height)")
    }
    let input = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
    input.expectsMediaDataInRealTime = false
    let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
        kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        kCVPixelBufferWidthKey as String: spec.width,
        kCVPixelBufferHeightKey as String: spec.height,
        kCVPixelBufferIOSurfacePropertiesKey as String: [String: Any](),
        kCVPixelBufferMetalCompatibilityKey as String: true,
    ])
    guard w.canAdd(input) else { return abort("internal", "writer refused the video input") }
    w.add(input)
    // T33: add the audio input here (before startWriting), fed from screen.mov's audio track(s)
    // through the speed `segments`.
    guard w.startWriting() else {
        return abort(isDiskFull(w.error) ? "disk-full" : "internal", "could not start the writer: \(w.error?.localizedDescription ?? "unknown error")")
    }
    w.startSession(atSourceTime: .zero)
    guard let pool = adaptor.pixelBufferPool else { return abort("internal", "no pixel buffer pool") }
    send(["event": "started", "id": id as Any, "total_frames": spec.frames])

    let startedAt = DispatchTime.now()
    var lastSent = DispatchTime.now()
    func elapsed(_ since: DispatchTime) -> Double { Double(DispatchTime.now().uptimeNanoseconds - since.uptimeNanoseconds) / 1e9 }
    func beat() {
        if elapsed(lastSent) >= 10 {
            send(["event": "heartbeat", "id": id as Any])
            lastSent = DispatchTime.now()
        }
    }
    var lastProgress = DispatchTime(uptimeNanoseconds: 0)

    for i in 0..<spec.frames {
        if job.isCancelled { return abort("cancelled", "Export cancelled") }
        var failure: (String, String)?
        autoreleasepool {
            let composed = composeExportFrame(i, lines: lines, takeURL: takeURL, style: style, spec: spec)
            guard let (image, canvas) = composed.frame else {
                failure = composed.failure
                return
            }
            do {
                var pb: CVPixelBuffer?
                guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &pb) == kCVReturnSuccess, let buffer = pb else {
                    failure = ("internal", "could not allocate a pixel buffer")
                    return
                }
                renderContext.render(image, to: buffer, bounds: canvas, colorSpace: renderSRGB)
                CVBufferSetAttachment(buffer, kCVImageBufferColorPrimariesKey, kCVImageBufferColorPrimaries_ITU_R_709_2, .shouldPropagate)
                CVBufferSetAttachment(buffer, kCVImageBufferTransferFunctionKey, kCVImageBufferTransferFunction_ITU_R_709_2, .shouldPropagate)
                CVBufferSetAttachment(buffer, kCVImageBufferYCbCrMatrixKey, kCVImageBufferYCbCrMatrix_ITU_R_709_2, .shouldPropagate)
                // Backpressure: the encoder drains at its own pace.
                while !input.isReadyForMoreMediaData {
                    if job.isCancelled || w.status == .failed { break }
                    Thread.sleep(forTimeInterval: 0.002)
                    beat()
                }
                if job.isCancelled { return }
                if w.status == .failed {
                    failure = (isDiskFull(w.error) ? "disk-full" : "internal", "encoding failed: \(w.error?.localizedDescription ?? "unknown error")")
                    return
                }
                if !adaptor.append(buffer, withPresentationTime: CMTime(value: Int64(i), timescale: Int32(spec.fps))) {
                    failure = (isDiskFull(w.error) ? "disk-full" : "internal", "encoding failed: \(w.error?.localizedDescription ?? "unknown error")")
                }
            }
        }
        if job.isCancelled { return abort("cancelled", "Export cancelled") }
        if let (code, message) = failure { return abort(code, message) }

        let done = i + 1
        if done == spec.frames || elapsed(lastProgress) >= 0.25 || lastProgress.uptimeNanoseconds == 0 {
            let spent = elapsed(startedAt)
            let eta = spent / Double(done) * Double(spec.frames - done)
            send(["event": "progress", "id": id as Any, "done_frames": done, "total_frames": spec.frames,
                  "eta": (eta * 10).rounded() / 10])
            lastProgress = DispatchTime.now()
            lastSent = lastProgress
        } else {
            beat()
        }
    }

    input.markAsFinished()
    let sem = DispatchSemaphore(value: 0)
    w.finishWriting { sem.signal() }
    while sem.wait(timeout: .now() + 5) == .timedOut { beat() }
    if job.isCancelled { return abort("cancelled", "Export cancelled") }
    guard w.status == .completed else {
        return abort(isDiskFull(w.error) ? "disk-full" : "internal", "could not finish the file: \(w.error?.localizedDescription ?? "unknown error")")
    }
    do {
        if fm.fileExists(atPath: spec.out) { try fm.removeItem(atPath: spec.out) }
        try fm.moveItem(atPath: partial, toPath: spec.out)
    } catch {
        return abort("internal", "could not write \(spec.out): \(error.localizedDescription)")
    }
    let bytes = ((try? fm.attributesOfItem(atPath: spec.out)[.size]) as? NSNumber)?.int64Value ?? 0
    log(String(format: "render: %d frames %dx%d %@ in %.1f s", spec.frames, spec.width, spec.height, spec.codec, elapsed(startedAt)))
    releaseExportSlot(job)
    reply(id, ["path": spec.out, "bytes": bytes, "duration": Double(spec.frames) / Double(spec.fps), "frames": spec.frames])
}

// MARK: shared by the MP4 and GIF paths

/// Frame `i`: read its params line, decode the take frame at the SOURCE time Rust computed and
/// compose it with the same code as `render.frame`. Exactly one of the two results is set.
private func composeExportFrame(_ i: Int, lines: LineReader, takeURL: URL, style: RenderStyle,
                                spec: ExportSpec) -> (frame: (CIImage, CGRect)?, failure: (String, String)?) {
    guard let line = lines.next() else {
        return (nil, ("invalid", "params_file ended at frame \(i) of \(spec.frames)"))
    }
    guard let obj = (try? JSONSerialization.jsonObject(with: line)) as? [String: Any],
          let t = num(obj["t"]), t.isFinite, let fx = FrameFx(request: obj) else {
        return (nil, ("invalid", "params_file line \(i + 1) is malformed"))
    }
    do {
        // sequential decode (Decode.swift): output frames ask for increasing source times, so after the
        // first frame the reader is kept open; a trim or speed cut (a jump) costs one generator frame
        let take = try takeFrameAuto(at: takeURL, t: t, mode: spec.generatorOnly ? .generator : .auto).frame
        guard let composed = composeStyledImage(take: take, style: style, project: spec.project, width: spec.width,
                                                height: spec.height, fx: fx) else {
            return (nil, ("internal", "could not compose frame \(i)"))
        }
        return (composed, nil)
    } catch let e as RenderError {
        return (nil, ("internal", e.message))
    } catch {
        return (nil, ("internal", "frame \(i) failed: \(error.localizedDescription)"))
    }
}

/// GIF delays in centiseconds: frame i ends at round((i+1)*100/fps) and lasts the difference to the
/// previous end. Same arithmetic as `export::gif_delays` in Rust (integer, round half up).
func gifDelays(frames: Int, fps: Int) -> [Int] {
    func end(_ i: Int) -> Int { (i * 100 + fps / 2) / fps }
    return (0..<frames).map { end($0 + 1) - end($0) }
}

// MARK: GIF

private func runGifExport(_ job: ExportJob, _ spec: ExportSpec) {
    let id = job.id
    let fm = FileManager.default
    let partial = spec.out + ".partial"
    func abort(_ code: String, _ message: String) {
        try? fm.removeItem(atPath: partial)
        releaseExportSlot(job)
        fail(id, code, message)
    }

    let projectFile = (spec.project as NSString).appendingPathComponent("project.json")
    guard let raw = fm.contents(atPath: projectFile) else { return abort("not-found", "project.json not found") }
    guard let json = (try? JSONSerialization.jsonObject(with: raw)) as? [String: Any] else {
        return abort("invalid", "project.json is not valid")
    }
    let file = ((json["sources"] as? [String: Any])?["screen"] as? [String: Any])?["file"] as? String ?? "screen.mov"
    guard !file.isEmpty, !file.contains("/"), !file.contains("..") else {
        return abort("invalid", "sources.screen.file must be a plain file name")
    }
    let takePath = (spec.project as NSString).appendingPathComponent(file)
    guard fm.fileExists(atPath: takePath) else { return abort("not-found", "\(file) not found") }
    guard let lines = LineReader(path: spec.paramsFile) else { return abort("not-found", "params_file could not be opened") }
    let style = RenderStyle(json: json)
    let takeURL = URL(fileURLWithPath: takePath)

    try? fm.createDirectory(atPath: (spec.out as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
    try? fm.removeItem(atPath: partial)

    guard let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: partial) as CFURL, UTType.gif.identifier as CFString,
                                                     spec.frames, nil) else {
        return abort("internal", "could not create \(partial)")
    }
    // Loop count 0 = forever.
    CGImageDestinationSetProperties(dest, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFLoopCount: 0]] as CFDictionary)
    let delays = gifDelays(frames: spec.frames, fps: spec.fps)
    send(["event": "started", "id": id as Any, "total_frames": spec.frames])

    let startedAt = DispatchTime.now()
    var lastSent = DispatchTime.now()
    func elapsed(_ since: DispatchTime) -> Double { Double(DispatchTime.now().uptimeNanoseconds - since.uptimeNanoseconds) / 1e9 }
    func beat() {
        if elapsed(lastSent) >= 10 {
            send(["event": "heartbeat", "id": id as Any])
            lastSent = DispatchTime.now()
        }
    }
    var lastProgress = DispatchTime(uptimeNanoseconds: 0)

    for i in 0..<spec.frames {
        if job.isCancelled { return abort("cancelled", "Export cancelled") }
        var failure: (String, String)?
        autoreleasepool {
            let composed = composeExportFrame(i, lines: lines, takeURL: takeURL, style: style, spec: spec)
            guard let (image, canvas) = composed.frame else {
                failure = composed.failure
                return
            }
            guard let cg = renderContext.createCGImage(image, from: canvas, format: .RGBA8, colorSpace: renderSRGB) else {
                failure = ("internal", "could not render frame \(i)")
                return
            }
            let delay = Double(delays[i]) / 100
            let props: [CFString: Any] = [kCGImagePropertyGIFDictionary: [
                kCGImagePropertyGIFDelayTime: delay,
                kCGImagePropertyGIFUnclampedDelayTime: delay,
            ]]
            CGImageDestinationAddImage(dest, cg, props as CFDictionary)
        }
        if job.isCancelled { return abort("cancelled", "Export cancelled") }
        if let (code, message) = failure { return abort(code, message) }

        let done = i + 1
        if done == spec.frames || elapsed(lastProgress) >= 0.25 || lastProgress.uptimeNanoseconds == 0 {
            let spent = elapsed(startedAt)
            let eta = spent / Double(done) * Double(spec.frames - done)
            send(["event": "progress", "id": id as Any, "done_frames": done, "total_frames": spec.frames,
                  "eta": (eta * 10).rounded() / 10])
            lastProgress = DispatchTime.now()
            lastSent = lastProgress
        } else {
            beat()
        }
    }

    // ImageIO encodes in finalize: run it on its own thread so heartbeats and `cancel` are served.
    let sem = DispatchSemaphore(value: 0)
    var finalized = false
    DispatchQueue.global(qos: .userInitiated).async {
        finalized = CGImageDestinationFinalize(dest)
        sem.signal()
    }
    while sem.wait(timeout: .now() + 1) == .timedOut { beat() }
    if job.isCancelled { return abort("cancelled", "Export cancelled") }
    guard finalized else {
        return abort(fm.fileExists(atPath: partial) ? "internal" : "disk-full", "could not write the GIF (is the disk full?)")
    }
    do {
        if fm.fileExists(atPath: spec.out) { try fm.removeItem(atPath: spec.out) }
        try fm.moveItem(atPath: partial, toPath: spec.out)
    } catch {
        return abort("internal", "could not write \(spec.out): \(error.localizedDescription)")
    }
    let bytes = ((try? fm.attributesOfItem(atPath: spec.out)[.size]) as? NSNumber)?.int64Value ?? 0
    let total = Double(delays.reduce(0, +)) / 100
    log(String(format: "render: %d frames %dx%d gif in %.1f s", spec.frames, spec.width, spec.height, elapsed(startedAt)))
    releaseExportSlot(job)
    reply(id, ["path": spec.out, "bytes": bytes, "duration": total, "frames": spec.frames])
}
