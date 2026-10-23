import AVFoundation
import CoreGraphics
import CoreMedia
import CoreVideo
import Foundation
import IOKit.pwr_mgt
import ScreenCaptureKit

// Recording core: SCStream -> AVAssetWriter (`screen.mov`).
//
// Timeline rule (S11): the video track length must equal the wall-clock time between the
// `record.start` reply and the stop request, even for a static screen. SCStream only delivers a
// frame when the screen changes, so the writer
//   - stamps the first frame at t = 0 (the screen as it was at start),
//   - subtracts paused time from every later timestamp (pause = time-shift, no gap),
//   - at stop appends the last frame again one frame before the end and ends the session at the
//     stop time, so a still screen still yields the full duration.
// Everything is measured on the host clock (the clock SCK stamps its samples with).

func hostSeconds() -> Double { CMClockGetTime(CMClockGetHostTimeClock()).seconds }

struct RecordFailure: Error { let message: String }

/// AVAssetWriter wrapper with the timeline rules above. Not thread-safe: the owner serialises calls.
/// Times are passed in (host seconds) so the logic can be driven without ScreenCaptureKit.
final class ScreenWriter {
    let partialURL: URL
    let finalURL: URL
    let width: Int
    let height: Int
    let fps: Int
    private(set) var frames = 0
    private(set) var dropped = 0
    private(set) var finished = false
    private let writer: AVAssetWriter
    private let input: AVAssetWriterInput
    private let adaptor: AVAssetWriterInputPixelBufferAdaptor
    private var t0 = 0.0
    private var pausedTotal = 0.0
    private var pausedAt: Double?
    private var lastPTS = -1.0
    private var lastBuffer: CVPixelBuffer?
    private let timescale: CMTimeScale = 100_000

    init(final: URL, width: Int, height: Int, fps: Int) throws {
        self.finalURL = final
        self.partialURL = URL(fileURLWithPath: final.path + ".partial")
        self.width = width
        self.height = height
        self.fps = fps
        try? FileManager.default.removeItem(at: partialURL)
        writer = try AVAssetWriter(outputURL: partialURL, fileType: .mov)
        let bitrate = min(100_000_000, max(2_000_000, Int(Double(width * height * fps) * 0.05)))
        let hevc: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.hevc, AVVideoWidthKey: width, AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: bitrate, AVVideoExpectedSourceFrameRateKey: fps,
                                              AVVideoAllowFrameReorderingKey: false],
        ]
        let h264: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: width, AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: bitrate, AVVideoExpectedSourceFrameRateKey: fps,
                                              AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel],
        ]
        let settings = writer.canApply(outputSettings: hevc, forMediaType: .video) ? hevc : h264
        input = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
        input.expectsMediaDataInRealTime = true
        adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: nil)
        guard writer.canAdd(input) else { throw RecordFailure(message: "cannot add the video input") }
        writer.add(input)
    }

    private func cm(_ seconds: Double) -> CMTime { CMTime(seconds: seconds, preferredTimescale: timescale) }

    /// Start the file; `at` is the host time that becomes t = 0.
    func begin(at: Double) throws {
        t0 = at
        guard writer.startWriting() else {
            throw RecordFailure(message: writer.error?.localizedDescription ?? "cannot start writing")
        }
        writer.startSession(atSourceTime: .zero)
    }

    /// Seconds of recorded (unpaused) time at host time `now`.
    func elapsed(at now: Double) -> Double { max(0, (pausedAt ?? now) - t0 - pausedTotal) }

    var failure: String? {
        writer.status == .failed ? (writer.error?.localizedDescription ?? "the writer failed") : nil
    }

    /// One captured frame taken at host time `hostTime`.
    func frame(_ pixels: CVPixelBuffer, hostTime: Double) {
        guard !finished, pausedAt == nil else { return }
        lastBuffer = pixels  // newest content, also used to pad the end
        let pts = max(0, hostTime - t0 - pausedTotal)
        if frames > 0 && pts <= lastPTS + 1e-6 { dropped += 1; return }
        guard input.isReadyForMoreMediaData, adaptor.append(pixels, withPresentationTime: cm(pts)) else {
            dropped += 1
            return
        }
        frames += 1
        lastPTS = pts
    }

    func pause(at now: Double) { if pausedAt == nil { pausedAt = now } }

    func resume(at now: Double) {
        if let p = pausedAt { pausedTotal += max(0, now - p) }
        pausedAt = nil
    }

    /// Remove the half-written file.
    func discard() {
        finished = true
        if writer.status == .writing { writer.cancelWriting() }
        try? FileManager.default.removeItem(at: partialURL)
    }

    /// Pad to the stop time, finish the file and rename it into place. Returns the duration.
    /// On any failure nothing is left on disk.
    func finish(at now: Double) throws -> Double {
        guard !finished else { throw RecordFailure(message: "already finished") }
        let end = elapsed(at: now)
        guard frames > 0, let last = lastBuffer else {
            discard()
            throw RecordFailure(message: "no frames were captured")
        }
        finished = true
        let interval = 1.0 / Double(fps)
        if end - interval > lastPTS + 1e-6, input.isReadyForMoreMediaData {
            _ = adaptor.append(last, withPresentationTime: cm(end - interval))
        }
        input.markAsFinished()
        writer.endSession(atSourceTime: cm(max(end, lastPTS)))
        let sem = DispatchSemaphore(value: 0)
        writer.finishWriting { sem.signal() }
        if sem.wait(timeout: .now() + 8) == .timedOut {
            discard()
            throw RecordFailure(message: "finishing the file timed out")
        }
        guard writer.status == .completed else {
            let why = writer.error?.localizedDescription ?? "the writer failed"
            try? FileManager.default.removeItem(at: partialURL)
            throw RecordFailure(message: why)
        }
        do {
            let fm = FileManager.default
            if fm.fileExists(atPath: finalURL.path) { try fm.removeItem(at: finalURL) }
            try fm.moveItem(at: partialURL, to: finalURL)
        } catch {
            try? FileManager.default.removeItem(at: partialURL)
            throw RecordFailure(message: "could not move the file into place: \(error.localizedDescription)")
        }
        return max(end, lastPTS)
    }
}

private func permissionDenied(_ id: Any?) -> Never {
    fail(id, "permission-screen", "Screen Recording is not granted")
    exit(1)
}

private func number(_ v: Any?) -> Double? { (v as? NSNumber)?.doubleValue }

private func even(_ v: Double) -> Int { max(2, Int(v.rounded()) & ~1) }

/// The recording in progress (one per helper process).
private var activeRecorder: Recorder?
private let recorderLock = NSLock()

private func currentRecorder() -> Recorder? {
    recorderLock.lock()
    defer { recorderLock.unlock() }
    return activeRecorder
}

final class Recorder: NSObject, SCStreamOutput, SCStreamDelegate {
    enum State { case arming, recording, paused, stopping, failed, done }

    let id: Any?
    let project: String
    let file: String
    let queue = DispatchQueue(label: "dev.ferrocast.capture.recorder")
    let writer: ScreenWriter
    var stream: SCStream?
    // Everything below is touched only on `queue`.
    var state = State.arming
    var pending: CVPixelBuffer?
    var timer: DispatchSourceTimer?
    var interruption: String?
    var outcome: Result<Double, RecordFailure>?
    var activity: NSObjectProtocol?
    var assertion = IOPMAssertionID(0)
    var lastDropped = 0
    var lastDroppedWarn = 0.0
    var lastDiskWarn = 0.0
    var warnedNoFrames = false
    /// Input event log (`events.json`), started and stopped with the recording.
    var events: InputRecorder?

    init(id: Any?, project: String, file: String, writer: ScreenWriter) {
        self.id = id
        self.project = project
        self.file = file
        self.writer = writer
    }

    private func event(_ name: String, _ fields: [String: Any] = [:]) {
        var o = fields
        o["event"] = name
        if let id = id { o["id"] = id }
        send(o)
    }

    // MARK: power

    /// Defeat App Nap and display sleep for the whole recording.
    func holdPower() {
        activity = ProcessInfo.processInfo.beginActivity(options: [.userInitiated, .latencyCritical], reason: "Ferrocast screen recording")
        let level = IOPMAssertionLevel(kIOPMAssertionLevelOn)
        let rc = IOPMAssertionCreateWithName(kIOPMAssertPreventUserIdleDisplaySleep as CFString, level,
                                             "Ferrocast screen recording" as CFString, &assertion)
        if rc != kIOReturnSuccess { log("could not hold the display-sleep assertion (\(rc))") }
    }

    func releasePower() {
        if let a = activity { ProcessInfo.processInfo.endActivity(a); activity = nil }
        if assertion != 0 { IOPMAssertionRelease(assertion); assertion = 0 }
    }

    // MARK: stream

    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, CMSampleBufferIsValid(sb) else { return }
        guard let atts = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let raw = atts.first?[.status] as? Int, SCFrameStatus(rawValue: raw) == .complete,
              let pixels = CMSampleBufferGetImageBuffer(sb) else { return }
        switch state {
        case .arming: pending = pixels
        case .recording: writer.frame(pixels, hostTime: CMSampleBufferGetPresentationTimeStamp(sb).seconds)
        default: break
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        queue.async { self.interrupt(error.localizedDescription) }
    }

    /// The stream ended without a stop request (display unplugged, permission revoked…): keep what
    /// was recorded as a finished file and report it, never a half-written one.
    private func interrupt(_ message: String) {
        guard state == .recording || state == .paused || state == .arming else { return }
        let wasArming = state == .arming
        state = .failed
        interruption = message
        timer?.cancel()
        timer = nil
        outcome = wasArming ? .failure(RecordFailure(message: message)) : finishFile(at: hostSeconds())
        if case .failure(let f) = outcome { log("recording lost: \(f.message)") }
        if case .success = outcome { events?.finish(at: hostSeconds(), write: true) } else { events?.finish(at: hostSeconds(), write: false) }
        releasePower()
        event("state", ["state": "failed", "message": message])
    }

    private func finishFile(at now: Double) -> Result<Double, RecordFailure> {
        do { return .success(try writer.finish(at: now)) } catch let f as RecordFailure { return .failure(f) } catch {
            return .failure(RecordFailure(message: error.localizedDescription))
        }
    }

    // MARK: lifecycle (callers are not on `queue`)

    /// Arm the writer once the stream runs: t = 0 is now and the latest frame already seen is its first.
    func arm() throws -> Double {
        var result: Result<Double, RecordFailure> = .failure(RecordFailure(message: "not armed"))
        queue.sync {
            guard state == .arming else { return }
            let t0 = hostSeconds()
            do {
                try writer.begin(at: t0)
                if let p = pending { writer.frame(p, hostTime: t0) }
                pending = nil
                state = .recording
                result = .success(t0)
            } catch let f as RecordFailure {
                result = .failure(f)
            } catch {
                result = .failure(RecordFailure(message: error.localizedDescription))
            }
        }
        return try result.get()
    }

    func startTicking() {
        let t = DispatchSource.makeTimerSource(queue: queue)
        t.schedule(deadline: .now() + 1, repeating: 1, leeway: .milliseconds(20))
        t.setEventHandler { [weak self] in self?.tick() }
        queue.sync { timer = t }
        t.resume()
        queue.async {
            self.event("state", ["state": "recording", "t": 0.0])
            if let e = self.events, !e.keysAvailable {
                self.event("warn", ["code": "input-monitoring-off", "message": "Keyboard shortcuts will not be recorded: Input Monitoring is off"])
            }
        }
    }

    private func tick() {
        guard state == .recording || state == .paused else { return }
        let now = hostSeconds()
        if let why = writer.failure {
            interrupt(why)
            return
        }
        let elapsed = (writer.elapsed(at: now) * 1000).rounded() / 1000
        event("tick", ["elapsed": elapsed, "frames": writer.frames, "dropped": writer.dropped])
        if writer.frames == 0 && elapsed > 5 && !warnedNoFrames {
            warnedNoFrames = true
            event("warn", ["code": "no-frames", "message": "no frame arrived in 5 s"])
        }
        if writer.dropped > lastDropped && now - lastDroppedWarn > 5 {
            lastDroppedWarn = now
            event("warn", ["code": "dropped-frames", "message": "\(writer.dropped - lastDropped) frames dropped"])
        }
        lastDropped = writer.dropped
        if now - lastDiskWarn > 30,
           let free = try? URL(fileURLWithPath: project).resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
               .volumeAvailableCapacityForImportantUsage, free < 2_000_000_000 {
            lastDiskWarn = now
            event("warn", ["code": "low-disk", "free_bytes": Int(free)])
        }
    }

    func pause() -> Bool {
        var ok = false
        queue.sync {
            guard state == .recording else { return }
            let now = hostSeconds()
            writer.pause(at: now)
            events?.pause(at: now)
            state = .paused
            ok = true
            event("state", ["state": "paused", "t": writer.elapsed(at: hostSeconds())])
        }
        return ok
    }

    func resume() -> Bool {
        var ok = false
        queue.sync {
            guard state == .paused else { return }
            let now = hostSeconds()
            writer.resume(at: now)
            events?.resume(at: now)
            state = .recording
            ok = true
            event("state", ["state": "recording", "t": writer.elapsed(at: hostSeconds())])
        }
        return ok
    }

    /// Stop the stream and finish the file. `stopTime` is when the stop was requested.
    func stop(stopTime: Double) -> (Result<Double, RecordFailure>, String?) {
        var interrupted: String?
        var alreadyDone = false
        queue.sync {
            timer?.cancel()
            timer = nil
            if state == .failed { alreadyDone = true; interrupted = interruption } else if state != .done { state = .stopping }
        }
        if !alreadyDone, let s = stream {
            let sem = DispatchSemaphore(value: 0)
            s.stopCapture { _ in sem.signal() }
            if sem.wait(timeout: .now() + 5) == .timedOut { log("stopCapture did not answer in 5 s") }
        }
        var result: Result<Double, RecordFailure> = .failure(RecordFailure(message: "already stopped"))
        queue.sync {
            if alreadyDone {
                result = outcome ?? result
            } else if state == .stopping {
                result = finishFile(at: stopTime)
                if case .success = result { events?.finish(at: stopTime, write: true) } else { events?.finish(at: stopTime, write: false) }
                state = .done
                releasePower()
                event("state", ["state": "stopped"])
            }
        }
        return (result, interrupted)
    }
}

/// `record.start`: begin writing `<project>/<file>`; replies once the stream runs.
@available(macOS 14, *)
func handleRecordStart(_ id: Any?, _ req: [String: Any]) {
    guard let project = req["project"] as? String, project.hasPrefix("/"),
          let target = req["target"] as? [String: Any], let kind = target["kind"] as? String else {
        fail(id, "invalid", "record.start needs an absolute project path and a target")
        return
    }
    let file = req["file"] as? String ?? "screen.mov"
    if file.isEmpty || file == "." || file == ".." || file.contains("/") || !file.hasSuffix(".mov") {
        fail(id, "invalid", "file must be a plain .mov file name")
        return
    }
    let fps = Int(number(req["fps"]) ?? 60)
    if fps < 1 || fps > 120 {
        fail(id, "invalid", "fps must be 1...120")
        return
    }
    if currentRecorder() != nil {
        fail(id, "busy", "a recording is already running")
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
    config.queueDepth = 6
    config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(fps))
    config.pixelFormat = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
    let excluded = Set(((req["excludes"] as? [String: Any])?["window_ids"] as? [NSNumber] ?? []).map { $0.uint32Value })
    let filter: SCContentFilter
    var points = CGSize.zero
    var inputDisplay: CGRect?
    var captureRect: CGRect?
    var inputWindow: UInt32?
    switch kind {
    case "display", "rect":
        guard let did = number(target["display"]),
              let display = content.displays.first(where: { Double($0.displayID) == did }) else {
            fail(id, "not-found", "display \(number(target["display"]) ?? -1) not found")
            return
        }
        filter = SCContentFilter(display: display, excludingWindows: content.windows.filter { excluded.contains($0.windowID) })
        points = display.frame.size
        inputDisplay = display.frame
        if kind == "rect" {
            guard let r = target["rect"] as? [Any], r.count == 4, let x = number(r[0]), let y = number(r[1]),
                  let w = number(r[2]), let h = number(r[3]), w >= 1, h >= 1 else {
                fail(id, "invalid", "rect must be [x, y, w, h] with positive size")
                return
            }
            let clamped = CGRect(x: x, y: y, width: w, height: h).intersection(CGRect(origin: .zero, size: points))
            if clamped.isNull || clamped.width < 1 || clamped.height < 1 {
                fail(id, "invalid", "rect is outside the display")
                return
            }
            config.sourceRect = clamped
            captureRect = clamped
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
        inputWindow = window.windowID
        // The display holding most of the window; positions in events.json are relative to it.
        let best = content.displays.max { a, b in
            let ia = a.frame.intersection(window.frame), ib = b.frame.intersection(window.frame)
            return (ia.isNull ? 0 : ia.width * ia.height) < (ib.isNull ? 0 : ib.width * ib.height)
        }
        inputDisplay = best?.frame ?? CGRect(origin: .zero, size: window.frame.size)
        if let d = best?.frame {
            captureRect = CGRect(x: window.frame.minX - d.minX, y: window.frame.minY - d.minY,
                                 width: window.frame.width, height: window.frame.height)
        }
    default:
        fail(id, "invalid", "unknown target kind: \(kind)")
        return
    }
    let scale = Double(filter.pointPixelScale)
    let width = even(Double(points.width) * scale)
    let height = even(Double(points.height) * scale)
    config.width = width
    config.height = height

    let projectURL = URL(fileURLWithPath: project, isDirectory: true)
    let writer: ScreenWriter
    do {
        try FileManager.default.createDirectory(at: projectURL, withIntermediateDirectories: true)
        writer = try ScreenWriter(final: projectURL.appendingPathComponent(file), width: width, height: height, fps: fps)
    } catch {
        fail(id, "internal", "cannot create the recording: \(error.localizedDescription)")
        return
    }

    let rec = Recorder(id: id, project: project, file: file, writer: writer)
    if let frame = inputDisplay {
        let screen = EventsScreen(size: (width, height), scale: scale, displayFrame: frame, captureRect: captureRect)
        rec.events = InputRecorder(config: EventsConfig(screen: screen, windowID: inputWindow, excluded: excluded),
                                   url: projectURL.appendingPathComponent("events.json"))
    }
    let stream = SCStream(filter: filter, configuration: config, delegate: rec)
    rec.stream = stream
    do {
        try stream.addStreamOutput(rec, type: .screen, sampleHandlerQueue: rec.queue)
    } catch {
        writer.discard()
        fail(id, "internal", "cannot attach the stream output: \(error.localizedDescription)")
        return
    }
    rec.holdPower()
    var startError: Error?
    stream.startCapture { e in
        startError = e
        sem.signal()
    }
    if sem.wait(timeout: .now() + 10) == .timedOut {
        writer.discard()
        rec.releasePower()
        fail(id, "internal", "the capture stream did not start")
        return
    }
    if let e = startError {
        writer.discard()
        rec.releasePower()
        if (e as NSError).code == -3801 { permissionDenied(id) }
        fail(id, "internal", e.localizedDescription)
        return
    }
    let t0: Double
    do { t0 = try rec.arm() } catch {
        rec.events = nil
        stream.stopCapture { _ in sem.signal() }
        _ = sem.wait(timeout: .now() + 5)
        writer.discard()
        rec.releasePower()
        fail(id, "internal", (error as? RecordFailure)?.message ?? error.localizedDescription)
        return
    }
    rec.events?.start(at: t0)
    recorderLock.lock()
    activeRecorder = rec
    recorderLock.unlock()
    reply(id, ["started_at_host": t0, "size": [width, height], "file": file])
    rec.startTicking()
}

@available(macOS 14, *)
func handleRecordControl(_ id: Any?, _ cmd: String) {
    let stopTime = hostSeconds()
    guard let rec = currentRecorder() else {
        fail(id, "invalid", "not recording")
        return
    }
    switch cmd {
    case "record.pause":
        rec.pause() ? reply(id, [:]) : fail(id, "invalid", "not recording or already paused")
    case "record.resume":
        rec.resume() ? reply(id, [:]) : fail(id, "invalid", "not paused")
    default:
        let (result, interrupted) = rec.stop(stopTime: stopTime)
        recorderLock.lock()
        activeRecorder = nil
        recorderLock.unlock()
        switch result {
        case .failure(let f):
            fail(id, "internal", f.message)
        case .success(let duration):
            var body: [String: Any] = [
                "project": rec.project, "duration": (duration * 1000).rounded() / 1000, "screen": rec.file,
                "size": [rec.writer.width, rec.writer.height], "frames": rec.writer.frames, "dropped": rec.writer.dropped,
            ]
            if let why = interrupted {
                body["interrupted"] = true
                body["message"] = why
            }
            reply(id, body)
        }
    }
}

/// Finish an active recording when the app goes away (stdin EOF) or the helper is terminated, so a
/// playable file is kept instead of a `.partial`.
func shutdownRecorder() {
    guard #available(macOS 14, *), let rec = currentRecorder() else { return }
    let (result, _) = rec.stop(stopTime: hostSeconds())
    if case .failure(let f) = result { log("recording could not be finished: \(f.message)") }
    recorderLock.lock()
    activeRecorder = nil
    recorderLock.unlock()
}
