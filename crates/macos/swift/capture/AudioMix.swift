import AVFoundation
import CoreMedia
import Foundation

// MP4 export audio (P4, S16, T33): `mic.m4a` and `system.m4a` mixed into ONE AAC track, 48 kHz stereo.
//
// Request field (additive; absent = no audio, exactly as before):
//   "audio": [{"track":"mic"|"system", "file":"mic.m4a", "gain":1.0,
//              "segments":[{"src_start":..,"src_end":..,"out_start":..,"speed":..}]}]
// Rust (`export::audio_inputs`, from `timeline::audio_segments` and `Audio::linear_gain`) owns every
// time and gain; the runs are in FILE time and OUTPUT time and muted tracks are never sent. Swift only
// places and sums them.
//
// Method (sample accurate, bounded memory): every run is read by an `AVAssetReader` limited to its
// `src_start..src_end` and converted by the reader to 48 kHz stereo Float32 (mono and other rates are
// converted by CoreAudio). Boundaries are rounded once from seconds to 48 kHz samples, exactly like
// `AudioSegment::to_samples` in Rust, so neighbouring runs share an edge. Output sample `p` of a run
// starting at `out_start` reads source position `src_start*48000 + (p - out_start) * speed`: speed 1 is a
// plain copy, any other speed is a linear-interpolated RATE change, so the pitch follows the speed
// (documented in `timeline.rs`; 2x = half the duration, one octave up). Gaps are silence, the end
// of a file is silence. The sum is clamped to [-1, 1]. The mix is as long as the video:
// `frames * 48000 / fps` samples.

let mixRate = 48_000

struct AudioRunSpec {
    let srcStart: Double
    let srcEnd: Double
    let outStart: Double
    let speed: Double
}

struct AudioInputSpec {
    let track: String
    let url: URL
    let gain: Double
    let runs: [AudioRunSpec]
}

/// Parse and validate the `audio` field of a `render` request. nil = absent; throws a message for a malformed one.
func parseAudioInputs(_ req: [String: Any], project: String) throws -> [AudioInputSpec] {
    guard let raw = req["audio"], !(raw is NSNull) else { return [] }
    guard let list = raw as? [[String: Any]] else { throw RenderError("render audio is malformed") }
    var out: [AudioInputSpec] = []
    for a in list {
        guard let track = a["track"] as? String, track == "mic" || track == "system",
              let file = a["file"] as? String, !file.isEmpty, !file.contains("/"), !file.contains(".."),
              let gain = num(a["gain"]), gain.isFinite, gain >= 0, gain <= 64,
              let segs = a["segments"] as? [[String: Any]] else { throw RenderError("render audio is malformed") }
        var runs: [AudioRunSpec] = []
        for s in segs {
            guard let a = num(s["src_start"]), let b = num(s["src_end"]), let o = num(s["out_start"]), let k = num(s["speed"]),
                  a.isFinite, b.isFinite, o.isFinite, k.isFinite, a >= 0, b > a, o >= 0, k > 0 else {
                throw RenderError("render audio segments are malformed")
            }
            runs.append(AudioRunSpec(srcStart: a, srcEnd: b, outStart: o, speed: k))
        }
        // a zero gain (muted) track contributes nothing: skipped, like an empty one
        if gain > 0, !runs.isEmpty {
            out.append(AudioInputSpec(track: track, url: URL(fileURLWithPath: (project as NSString).appendingPathComponent(file)),
                                      gain: gain, runs: runs.sorted { $0.outStart < $1.outStart }))
        }
    }
    return out
}

/// Interleaved stereo Float32 source samples of one run, read from an `AVAssetReader`.
private final class RunSource {
    private let reader: AVAssetReader
    private let output: AVAssetReaderTrackOutput
    /// Interleaved L,R of the frames from `base` on.
    private var window: [Float] = []
    private var base: Int64 = 0
    private var started = false
    private var eof = false

    init?(asset: AVURLAsset, track: AVAssetTrack, start: Double, end: Double) {
        guard let r = try? AVAssetReader(asset: asset) else { return nil }
        let out = AVAssetReaderTrackOutput(track: track, outputSettings: [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: mixRate,
            AVNumberOfChannelsKey: 2,
            AVLinearPCMBitDepthKey: 32,
            AVLinearPCMIsFloatKey: true,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false,
        ])
        out.alwaysCopiesSampleData = false
        // a little past the end: the linear interpolation of the last output sample reads one frame ahead
        r.timeRange = CMTimeRange(start: CMTime(seconds: start, preferredTimescale: 48_000),
                                  end: CMTime(seconds: end + 0.02, preferredTimescale: 48_000))
        guard r.canAdd(out) else { return nil }
        r.add(out)
        guard r.startReading() else { return nil }
        reader = r
        output = out
    }

    func cancel() { reader.cancelReading() }

    private func readMore() {
        guard let sb = output.copyNextSampleBuffer() else { eof = true; return }
        let n = CMSampleBufferGetNumSamples(sb)
        guard n > 0, let block = CMSampleBufferGetDataBuffer(sb) else { return }
        var samples = [Float](repeating: 0, count: n * 2)
        let bytes = min(CMBlockBufferGetDataLength(block), n * 2 * MemoryLayout<Float>.size)
        _ = samples.withUnsafeMutableBytes { CMBlockBufferCopyDataBytes(block, atOffset: 0, dataLength: bytes, destination: $0.baseAddress!) }
        let at = Int64((CMSampleBufferGetPresentationTimeStamp(sb).seconds * Double(mixRate)).rounded())
        if !started {
            started = true
            base = at
            window = samples
            return
        }
        let end = base + Int64(window.count / 2)
        if at > end { window.append(contentsOf: [Float](repeating: 0, count: Int(at - end) * 2)) }
        let skip = max(0, Int(end - at))
        if skip < n { window.append(contentsOf: samples[(skip * 2)...]) }
    }

    /// Make frames up to and including `index` available (or reach the end of the file).
    func ensure(_ index: Int64) {
        while !eof, !started || base + Int64(window.count / 2) <= index { readMore() }
    }

    /// Drop frames before `index`.
    func trim(before index: Int64) {
        let drop = Int(min(max(index - base, 0), Int64(window.count / 2)))
        if drop > 8192 {
            window.removeFirst(drop * 2)
            base += Int64(drop)
        }
    }

    func frame(_ index: Int64) -> (Float, Float) {
        let i = Int(index - base)
        guard i >= 0, i * 2 + 1 < window.count else { return (0, 0) }
        return (window[i * 2], window[i * 2 + 1])
    }
}

private final class MixInput {
    let gain: Float
    let runs: [(outStart: Int64, outEnd: Int64, srcStart: Int64, srcEnd: Double, speed: Double, seconds: (Double, Double))]
    let asset: AVURLAsset
    let track: AVAssetTrack
    var current = -1
    var source: RunSource?

    init(spec: AudioInputSpec, asset: AVURLAsset, track: AVAssetTrack) {
        gain = Float(spec.gain)
        self.asset = asset
        self.track = track
        let r = Double(mixRate)
        runs = spec.runs.map { run in
            let outEnd = run.outStart + (run.srcEnd - run.srcStart) / run.speed
            return (Int64((run.outStart * r).rounded()), Int64((outEnd * r).rounded()), Int64((run.srcStart * r).rounded()),
                    run.srcEnd * r, run.speed, (run.srcStart, run.srcEnd))
        }
    }
}

final class AudioMixer {
    let total: Int64
    private(set) var position: Int64 = 0
    private var inputs: [MixInput] = []

    /// Throws a `RenderError` when a file is missing or has no audio.
    init(specs: [AudioInputSpec], frames: Int, fps: Int) throws {
        total = Int64((Double(frames) * Double(mixRate) / Double(fps)).rounded())
        for s in specs {
            guard FileManager.default.fileExists(atPath: s.url.path) else { throw RenderError("\(s.url.lastPathComponent) not found") }
            let asset = AVURLAsset(url: s.url)
            guard let track = try? blocking({ try await asset.loadTracks(withMediaType: .audio).first }) else {
                throw RenderError("\(s.url.lastPathComponent) has no audio track")
            }
            inputs.append(MixInput(spec: s, asset: asset, track: track))
        }
    }

    var finished: Bool { position >= total }

    /// `next` runs on the writer's audio queue while `close` may come from the export thread (cancel).
    private let lock = NSLock()
    private var closed = false

    func close() {
        lock.lock()
        defer { lock.unlock() }
        closed = true
        for i in inputs {
            i.source?.cancel()
            i.source = nil
        }
    }

    /// The next `count` frames (fewer at the end) as interleaved stereo Float32, or nil at the end.
    func next(_ count: Int) -> (samples: [Float], start: Int64)? {
        lock.lock()
        defer { lock.unlock() }
        let n = Int(min(Int64(count), total - position))
        guard n > 0, !closed else { return nil }
        var mix = [Float](repeating: 0, count: n * 2)
        let start = position
        for input in inputs { add(input, into: &mix, start: start, count: n) }
        for i in 0..<mix.count { mix[i] = min(max(mix[i], -1), 1) }
        position += Int64(n)
        return (mix, start)
    }

    private func add(_ input: MixInput, into mix: inout [Float], start: Int64, count: Int) {
        var p = start
        let end = start + Int64(count)
        while p < end {
            // the run covering p, or the next one
            guard let idx = input.runs.firstIndex(where: { $0.outEnd > p }) else { return }
            let run = input.runs[idx]
            if run.outStart > p {
                p = run.outStart
                continue
            }
            if input.current != idx {
                input.source?.cancel()
                input.source = RunSource(asset: input.asset, track: input.track, start: run.seconds.0, end: run.seconds.1)
                input.current = idx
                if input.source == nil { log("audio: could not read \(input.asset.url.lastPathComponent)") }
            }
            let stop = min(end, run.outEnd)
            if let src = input.source {
                while p < stop {
                    let pos = Double(run.srcStart) + Double(p - run.outStart) * run.speed
                    let i0 = Int64(pos.rounded(.down))
                    let frac = Float(pos - Double(i0))
                    src.ensure(i0 + 1)
                    let l0 = src.frame(i0), l1 = src.frame(i0 + 1)
                    let o = Int(p - start) * 2
                    if frac == 0 {
                        mix[o] += l0.0 * input.gain
                        mix[o + 1] += l0.1 * input.gain
                    } else {
                        mix[o] += (l0.0 + (l1.0 - l0.0) * frac) * input.gain
                        mix[o + 1] += (l0.1 + (l1.1 - l0.1) * frac) * input.gain
                    }
                    p += 1
                    if p & 0x3FF == 0 { src.trim(before: i0 - 2) }
                }
            } else {
                p = stop
            }
        }
    }
}

/// An interleaved stereo Float32 chunk as a sample buffer the AAC writer input accepts.
func audioSampleBuffer(samples: [Float], start: Int64) -> CMSampleBuffer? {
    var asbd = AudioStreamBasicDescription(
        mSampleRate: Double(mixRate), mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked, mBytesPerPacket: 8, mFramesPerPacket: 1,
        mBytesPerFrame: 8, mChannelsPerFrame: 2, mBitsPerChannel: 32, mReserved: 0)
    var format: CMAudioFormatDescription?
    guard CMAudioFormatDescriptionCreate(allocator: nil, asbd: &asbd, layoutSize: 0, layout: nil, magicCookieSize: 0,
                                         magicCookie: nil, extensions: nil, formatDescriptionOut: &format) == noErr,
          let format else { return nil }
    let bytes = samples.count * MemoryLayout<Float>.size
    var block: CMBlockBuffer?
    guard CMBlockBufferCreateWithMemoryBlock(allocator: nil, memoryBlock: nil, blockLength: bytes, blockAllocator: nil,
                                             customBlockSource: nil, offsetToData: 0, dataLength: bytes, flags: 0,
                                             blockBufferOut: &block) == noErr, let block else { return nil }
    guard samples.withUnsafeBytes({ CMBlockBufferReplaceDataBytes(with: $0.baseAddress!, blockBuffer: block, offsetIntoDestination: 0, dataLength: bytes) }) == noErr
    else { return nil }
    var sb: CMSampleBuffer?
    guard CMAudioSampleBufferCreateReadyWithPacketDescriptions(
        allocator: nil, dataBuffer: block, formatDescription: format, sampleCount: samples.count / 2,
        presentationTimeStamp: CMTime(value: start, timescale: CMTimeScale(mixRate)), packetDescriptions: nil,
        sampleBufferOut: &sb) == noErr else { return nil }
    return sb
}
