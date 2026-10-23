import AVFoundation
import CoreMedia
import CoreVideo
import Foundation

// Side sources of a recording (P4): camera.mov, mic.m4a, system.m4a.
//
// Every source is written by a `TrackWriter` with the same rules as the screen writer:
//   - `<file>.partial`, renamed on a clean finish; nothing is left behind on failure,
//   - time stamps are HOST-clock seconds (the clock `hostSeconds()` and the screen writer use), so a
//     source's position against the screen take is exact, whatever clock the device stamps with,
//   - paused time is subtracted (time-shift, no gap),
//   - the file's time 0 is its first accepted sample; `offset` is the distance, in seconds on the
//     screen timeline, from the screen take's t0 to that sample (positive: the source started later;
//     a mic/system buffer that straddles t0 gives a small negative offset),
//   - at the end the session is closed so that `duration + offset` equals the screen duration.
// The writer is created lazily from the first sample (it needs the real size / sample format), so a
// source that never delivers leaves no file at all.

enum MediaKind: String {
    case camera, microphone

    var type: AVMediaType { self == .camera ? .video : .audio }

    /// The helper error code of S12.
    var permissionCode: String { self == .camera ? "permission-camera" : "permission-microphone" }

    var label: String { self == .camera ? "Camera" : "Microphone" }

    /// `authorized`, `denied`, `restricted` or `notDetermined`. Never prompts.
    var access: String {
        switch AVCaptureDevice.authorizationStatus(for: type) {
        case .authorized: return "authorized"
        case .denied: return "denied"
        case .restricted: return "restricted"
        case .notDetermined: return "notDetermined"
        @unknown default: return "denied"
        }
    }
}

/// Seconds on the host clock for a sample time stamped by `clock` (nil = already the host clock).
func hostSeconds(of pts: CMTime, clock: CMClock?) -> Double {
    let host = CMClockGetHostTimeClock()
    guard let clock = clock, !CFEqual(clock, host) else { return pts.seconds }
    return CMSyncConvertTime(pts, from: clock, to: host).seconds
}

struct TrackInfo {
    let file: String
    let offset: Double
    let size: [Int]
    let frames: Int

    var json: [String: Any] { ["file": file, "offset": (offset * 1000).rounded() / 1000] }
    var cameraJSON: [String: Any] {
        var o = json
        o["size"] = size
        o["frames"] = frames
        return o
    }
}

final class TrackWriter {
    enum Kind { case video(fps: Int), audio }

    let kind: Kind
    let finalURL: URL
    let partialURL: URL
    private let lock = NSLock()
    private var writer: AVAssetWriter?
    private var input: AVAssetWriterInput?
    private var adaptor: AVAssetWriterInputPixelBufferAdaptor?
    private var t0: Double?
    private var pausedAt: Double?
    private var pausedTotal = 0.0
    private var pausedTotalAtFirst = 0.0
    private var firstHost = 0.0
    private var offsetValue = 0.0
    private var lastPTS = -1.0  // video: last frame; audio: end of the last buffer
    private var lastBuffer: CVPixelBuffer?
    private var frameCount = 0
    private var dropCount = 0
    private var width = 0
    private var height = 0
    private var lostAtElapsed: Double?
    private var finished = false
    private var broken: String?
    private var lastSampleHost = 0.0

    init(final: URL, kind: Kind) {
        self.kind = kind
        self.finalURL = final
        self.partialURL = URL(fileURLWithPath: final.path + ".partial")
    }

    private var timescale: CMTimeScale {
        if case .audio = kind { return 48_000 }
        return 100_000
    }
    private func cm(_ seconds: Double) -> CMTime { CMTime(seconds: seconds, preferredTimescale: timescale) }

    // MARK: control (any thread)

    func arm(at host: Double) { lock.lock(); t0 = host; lock.unlock() }
    func pause(at host: Double) { lock.lock(); if pausedAt == nil { pausedAt = host }; lock.unlock() }
    func resume(at host: Double) {
        lock.lock()
        if let p = pausedAt { pausedTotal += max(0, host - p) }
        pausedAt = nil
        lock.unlock()
    }

    /// The device went away: keep what exists, accept nothing more, and end the file here.
    func markLost(at host: Double) {
        lock.lock()
        if lostAtElapsed == nil, let t0 = t0 { lostAtElapsed = max(0, (pausedAt ?? host) - t0 - pausedTotal) }
        lock.unlock()
    }

    var isLost: Bool { lock.lock(); defer { lock.unlock() }; return lostAtElapsed != nil }
    var frames: Int { lock.lock(); defer { lock.unlock() }; return frameCount }
    var sampleHostLast: Double { lock.lock(); defer { lock.unlock() }; return lastSampleHost }
    var failure: String? { lock.lock(); defer { lock.unlock() }; return broken }

    // MARK: samples

    /// One sample whose first frame / frame time is `host` (host-clock seconds).
    func append(_ sb: CMSampleBuffer, host: Double) {
        lock.lock()
        defer { lock.unlock() }
        guard let t0 = t0, !finished, broken == nil, pausedAt == nil, lostAtElapsed == nil, CMSampleBufferIsValid(sb),
              CMSampleBufferGetNumSamples(sb) > 0 else { return }
        let rawDuration = CMSampleBufferGetDuration(sb).seconds
        let duration = rawDuration.isFinite && rawDuration > 0 ? rawDuration : 0
        switch kind {
        case .video: guard host >= t0 else { return }
        case .audio: guard host + duration > t0 else { return }
        }
        lastSampleHost = host
        if writer == nil && !open(sb, host: host, t0: t0) { return }
        let pts = host - firstHost - (pausedTotal - pausedTotalAtFirst)
        guard let input = input else { return }
        switch kind {
        case .video:
            guard let pixels = CMSampleBufferGetImageBuffer(sb), let adaptor = adaptor else { return }
            lastBuffer = pixels
            if frameCount > 0 && pts <= lastPTS + 1e-6 { dropCount += 1; return }
            guard input.isReadyForMoreMediaData, adaptor.append(pixels, withPresentationTime: cm(max(0, pts))) else {
                dropCount += 1
                return
            }
            frameCount += 1
            lastPTS = max(0, pts)
        case .audio:
            // Overlap after a resume or timestamp jitter: never go backwards.
            if pts < lastPTS - 0.05 { dropCount += 1; return }
            let start = max(pts, lastPTS, 0)
            guard input.isReadyForMoreMediaData, let timed = retimed(sb, to: start) else { dropCount += 1; return }
            guard input.append(timed) else { dropCount += 1; return }
            frameCount += 1
            lastPTS = start + duration
        }
    }

    private func retimed(_ sb: CMSampleBuffer, to start: Double) -> CMSampleBuffer? {
        var count = 0
        CMSampleBufferGetSampleTimingInfoArray(sb, entryCount: 0, arrayToFill: nil, entriesNeededOut: &count)
        guard count > 0 else { return nil }
        var infos = [CMSampleTimingInfo](repeating: CMSampleTimingInfo(), count: count)
        CMSampleBufferGetSampleTimingInfoArray(sb, entryCount: count, arrayToFill: &infos, entriesNeededOut: &count)
        let base = infos[0].presentationTimeStamp
        for i in infos.indices {
            let delta = CMTimeSubtract(infos[i].presentationTimeStamp, base)
            infos[i].presentationTimeStamp = CMTimeAdd(cm(start), delta)
            infos[i].decodeTimeStamp = .invalid
        }
        var out: CMSampleBuffer?
        let rc = CMSampleBufferCreateCopyWithNewTiming(allocator: kCFAllocatorDefault, sampleBuffer: sb, sampleTimingEntryCount: count,
                                                       sampleTimingArray: infos, sampleBufferOut: &out)
        return rc == noErr ? out : nil
    }

    /// Create the file from the first sample. Locked by the caller.
    private func open(_ sb: CMSampleBuffer, host: Double, t0: Double) -> Bool {
        guard let format = CMSampleBufferGetFormatDescription(sb) else { broken = "sample without a format"; return false }
        try? FileManager.default.removeItem(at: partialURL)
        do {
            let w = try AVAssetWriter(outputURL: partialURL, fileType: { if case .video = kind { return .mov } else { return .m4a } }())
            let settings: [String: Any]
            switch kind {
            case .video(let fps):
                let dims = CMVideoFormatDescriptionGetDimensions(format)
                width = Int(dims.width)
                height = Int(dims.height)
                let bitrate = max(2_000_000, min(20_000_000, Int(Double(width * height * fps) * 0.1)))
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
                settings = w.canApply(outputSettings: hevc, forMediaType: .video) ? hevc : h264
            case .audio:
                // 48 kHz AAC; mono stays mono, anything else is written as stereo.
                var channels = 1
                if let asbd = CMAudioFormatDescriptionGetStreamBasicDescription(format)?.pointee { channels = asbd.mChannelsPerFrame >= 2 ? 2 : 1 }
                settings = [AVFormatIDKey: kAudioFormatMPEG4AAC, AVSampleRateKey: 48_000, AVNumberOfChannelsKey: channels,
                            AVEncoderBitRateKey: channels == 1 ? 96_000 : 128_000]
            }
            let mediaType: AVMediaType = { if case .video = kind { return .video } else { return .audio } }()
            let i = AVAssetWriterInput(mediaType: mediaType, outputSettings: settings, sourceFormatHint: { if case .audio = kind { return format } else { return nil } }())
            i.expectsMediaDataInRealTime = true
            guard w.canAdd(i) else { broken = "cannot add the input"; return false }
            w.add(i)
            if case .video = kind {
                adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: i, sourcePixelBufferAttributes: nil)
            }
            guard w.startWriting() else { broken = w.error?.localizedDescription ?? "cannot start writing"; return false }
            w.startSession(atSourceTime: .zero)
            writer = w
            input = i
            firstHost = host
            pausedTotalAtFirst = pausedTotal
            offsetValue = host - t0 - pausedTotal
            return true
        } catch {
            broken = error.localizedDescription
            return false
        }
    }

    // MARK: end

    /// Remove the half-written file.
    func discard() {
        lock.lock()
        defer { lock.unlock() }
        finished = true
        if let w = writer, w.status == .writing { w.cancelWriting() }
        try? FileManager.default.removeItem(at: partialURL)
    }

    /// Close the file so that `duration + offset` is `screenElapsed` (or the moment the source was lost)
    /// and rename it into place. `nil`: no sample ever arrived, no file exists. Throws on a writer error
    /// (nothing is left on disk then).
    func finish(screenElapsed: Double) throws -> TrackInfo? {
        lock.lock()
        defer { lock.unlock() }
        guard !finished else { return nil }
        finished = true
        guard let w = writer, let input = input, frameCount > 0 else {
            if let w = writer, w.status == .writing { w.cancelWriting() }
            try? FileManager.default.removeItem(at: partialURL)
            if let why = broken { throw RecordFailure(message: why) }
            return nil
        }
        var end = screenElapsed
        if let lost = lostAtElapsed { end = min(end, lost) }
        end = max(end - offsetValue, lastPTS)
        if case .video(let fps) = kind, let last = lastBuffer, let adaptor = adaptor {
            // Same as the screen take: a sparse source still covers the whole take.
            let interval = 1.0 / Double(fps)
            if end - interval > lastPTS + 1e-6, input.isReadyForMoreMediaData {
                _ = adaptor.append(last, withPresentationTime: cm(end - interval))
            }
        }
        input.markAsFinished()
        w.endSession(atSourceTime: cm(end))
        let sem = DispatchSemaphore(value: 0)
        w.finishWriting { sem.signal() }
        if sem.wait(timeout: .now() + 8) == .timedOut {
            w.cancelWriting()
            try? FileManager.default.removeItem(at: partialURL)
            throw RecordFailure(message: "finishing \(finalURL.lastPathComponent) timed out")
        }
        guard w.status == .completed else {
            let why = w.error?.localizedDescription ?? "the writer failed"
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
        return TrackInfo(file: finalURL.lastPathComponent, offset: offsetValue, size: [width, height], frames: frameCount)
    }
}

/// The optional sources of one recording and their lifecycle. Created by `record.start`.
final class SideSources {
    var camera: CameraCapture?
    var mic: MicCapture?
    var system: TrackWriter?
    /// Called with `camera-lost` / `mic-lost` / `system-audio-lost`-style codes and a message.
    var onWarn: ((String, String) -> Void)?

    var isEmpty: Bool { camera == nil && mic == nil && system == nil }
    private var writers: [TrackWriter] { [camera?.track, mic?.track, system].compactMap { $0 } }

    func arm(at t0: Double) { writers.forEach { $0.arm(at: t0) } }
    func pause(at now: Double) { writers.forEach { $0.pause(at: now) } }
    func resume(at now: Double) { writers.forEach { $0.resume(at: now) } }

    func stopSessions() {
        camera?.stop()
        mic?.stop()
    }

    func discardAll() {
        stopSessions()
        writers.forEach { $0.discard() }
    }

    /// A device that vanished or stopped delivering: warn once, keep the file.
    func poll(now: Double) {
        if let c = camera, c.checkLost(now: now) { onWarn?("camera-lost", "The camera stopped; the recording continues without it") }
        if let m = mic, m.checkLost(now: now) { onWarn?("mic-lost", "The microphone stopped; the recording continues without it") }
    }

    /// Finish every file. A source that fails here is reported in `warnings` and omitted; the rest is kept.
    func finish(screenElapsed: Double) -> (sources: [String: Any], warnings: [String]) {
        var out: [String: Any] = [:]
        var warnings: [String] = []
        func one(_ name: String, _ track: TrackWriter?, camera: Bool = false) {
            guard let t = track else { return }
            do {
                if let info = try t.finish(screenElapsed: screenElapsed) { out[name] = camera ? info.cameraJSON : info.json } else {
                    warnings.append("\(name): no data was recorded")
                }
            } catch {
                warnings.append("\(name): \((error as? RecordFailure)?.message ?? error.localizedDescription)")
            }
        }
        one("camera", camera?.track, camera: true)
        one("mic", mic?.track)
        one("system", system)
        return (out, warnings)
    }
}
