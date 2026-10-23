import AVFoundation
import Foundation
import Vision

// `analyze.face` (P6): on-device face tracking of the camera file of a project.
//
// Request  {"id":N,"cmd":"analyze.face","project":"/abs/Demo.ferrocast","fps":10}
//   `fps` (optional, 1...30, default 10) is the sampling rate. The camera file name comes from
//   `<project>/project.json` (`sources.camera.file`): a plain file name inside the project.
// Events   {"event":"progress","id":N,"done_secs":D,"total_secs":T}  (at most ~4/s)
//          {"event":"heartbeat","id":N}                                (after 10 s without any other line)
// Reply    {"id":N,"ok":true,"faces":N,"frames":M,"path":"/abs/Demo.ferrocast/camera_track.json"}
// Errors   invalid | not-found | busy | cancelled | disk-full | internal. Vision needs no permission.
// Output   <project>/camera_track.json =
//            {"version":1,"fps":10,"width":W,"height":H,"samples":[{"t","x","y","w","h","confidence"}]}
//          x,y,w,h normalised to the (upright) camera frame, ORIGIN TOP-LEFT (Vision's boxes are bottom-left
//          and are converted). Only the LARGEST face of a sampled frame is kept; frames without a face are
//          omitted. `t` is CAMERA FILE time in seconds (the camera source's offset is applied by the core).
//          Written to camera_track.json.partial and renamed; failure and cancel leave neither file.
// Decoding is sequential through AVAssetReader (one frame in flight), scaled by the reader to <= 640 px wide.
// Threading: own thread; stdin (`cancel`) keeps being served. One analysis at a time.

private let analyzeLock = NSLock()
private var activeAnalyze: TranscribeJob?  // same progress / heartbeat / cancel bookkeeping as a transcription

private let analyzeMaxWidth = 640.0

private func fileOrientation(_ t: CGAffineTransform) -> CGImagePropertyOrientation {
    switch (Int(t.a.rounded()), Int(t.b.rounded()), Int(t.c.rounded()), Int(t.d.rounded())) {
    case (0, 1, -1, 0): return .right
    case (0, -1, 1, 0): return .left
    case (-1, 0, 0, -1): return .down
    default: return .up
    }
}

func handleAnalyzeFace(_ id: Any?, _ req: [String: Any]) {
    guard let project = req["project"] as? String, project.hasPrefix("/") else {
        fail(id, "invalid", "analyze.face needs an absolute project path")
        return
    }
    var fps = 10.0
    if let raw = req["fps"] {
        guard let n = (raw as? NSNumber)?.doubleValue, !(raw is Bool), n.isFinite, n >= 1, n <= 30 else {
            fail(id, "invalid", "fps must be a number from 1 to 30")
            return
        }
        fps = n
    }
    var isDir: ObjCBool = false
    guard FileManager.default.fileExists(atPath: project, isDirectory: &isDir), isDir.boolValue else {
        fail(id, "not-found", "project folder not found")
        return
    }
    let projectURL = URL(fileURLWithPath: project)
    guard let data = try? Data(contentsOf: projectURL.appendingPathComponent("project.json")),
          let doc = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        fail(id, "not-found", "project.json is not in the project or is not readable")
        return
    }
    guard let camera = (doc["sources"] as? [String: Any])?["camera"] as? [String: Any],
          let cameraName = camera["file"] as? String else {
        fail(id, "not-found", "the project has no camera")
        return
    }
    guard isPlainFileName(cameraName) else {
        fail(id, "invalid", "the camera file must be a plain file name inside the project")
        return
    }
    let cameraURL = projectURL.appendingPathComponent(cameraName)
    guard FileManager.default.fileExists(atPath: cameraURL.path) else {
        fail(id, "not-found", "\(cameraName) is not in the project")
        return
    }
    let job = TranscribeJob(id: id)
    analyzeLock.lock()
    if activeAnalyze != nil {
        analyzeLock.unlock()
        fail(id, "busy", "a face analysis is already running")
        return
    }
    activeAnalyze = job
    analyzeLock.unlock()

    let samplingFps = fps
    let thread = Thread {
        defer {
            analyzeLock.lock()
            if activeAnalyze === job { activeAnalyze = nil }
            analyzeLock.unlock()
            job.finished.signal()
        }
        let out = projectURL.appendingPathComponent("camera_track.json")
        let partial = projectURL.appendingPathComponent("camera_track.json.partial")
        do {
            let result = try runFaceAnalysis(job: job, camera: cameraURL, fps: samplingFps)
            if job.isCancelled { throw TranscribeFailure(code: "cancelled", message: "Face tracking cancelled") }
            let body: [String: Any] = [
                "version": 1, "fps": samplingFps, "width": result.width, "height": result.height,
                "samples": result.samples.map {
                    ["t": $0.t, "x": $0.x, "y": $0.y, "w": $0.w, "h": $0.h, "confidence": $0.confidence] as [String: Any]
                },
            ]
            do {
                let json = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])
                try json.write(to: partial)
                guard rename(partial.path, out.path) == 0 else {
                    throw TranscribeFailure(code: "internal", message: "could not write camera_track.json: \(String(cString: strerror(errno)))")
                }
            } catch let e as TranscribeFailure {
                try? FileManager.default.removeItem(at: partial)
                throw e
            } catch {
                try? FileManager.default.removeItem(at: partial)
                let full = (error as NSError).code == NSFileWriteOutOfSpaceError
                throw TranscribeFailure(code: full ? "disk-full" : "internal", message: "could not write camera_track.json: \(error.localizedDescription)")
            }
            reply(id, ["faces": result.samples.count, "frames": result.frames, "path": out.path])
        } catch let e as TranscribeFailure {
            try? FileManager.default.removeItem(at: partial)
            fail(id, e.code, e.message)
        } catch {
            try? FileManager.default.removeItem(at: partial)
            fail(id, "internal", "face tracking failed: \(error.localizedDescription)")
        }
    }
    thread.qualityOfService = .userInitiated
    thread.start()
}

/// `cancel` for a face analysis: true when `target` is the running one (the reply was sent).
func handleAnalyzeCancel(_ id: Any?, _ req: [String: Any]) -> Bool {
    let target = (req["target"] as? NSNumber)?.intValue
    analyzeLock.lock()
    let job = activeAnalyze
    analyzeLock.unlock()
    guard let job, job.numericId == target else { return false }
    job.cancel()
    reply(id, [:])
    return true
}

/// Stdin closed or SIGTERM: stop and wait (bounded) for the thread to remove its partial file.
func shutdownAnalyze() {
    analyzeLock.lock()
    let job = activeAnalyze
    analyzeLock.unlock()
    guard let job else { return }
    job.cancel()
    _ = job.finished.wait(timeout: .now() + 10)
}

private struct FaceSample {
    var t: Double
    var x: Double
    var y: Double
    var w: Double
    var h: Double
    var confidence: Double
}

private struct FaceAnalysis {
    var width: Int
    var height: Int
    var frames: Int
    var samples: [FaceSample]
}

private func round4(_ v: Double) -> Double { (v * 10_000).rounded() / 10_000 }

private func runFaceAnalysis(job: TranscribeJob, camera: URL, fps: Double) throws -> FaceAnalysis {
    let asset = AVURLAsset(url: camera)
    guard let track = asset.tracks(withMediaType: .video).first else {
        throw TranscribeFailure(code: "invalid", message: "\(camera.lastPathComponent) has no video track")
    }
    let total = asset.duration.seconds
    guard total.isFinite, total > 0 else {
        throw TranscribeFailure(code: "invalid", message: "\(camera.lastPathComponent) has no duration")
    }
    let transform = track.preferredTransform
    let orientation = fileOrientation(transform)
    let natural = track.naturalSize
    let rotated = orientation == .left || orientation == .right
    let upright = rotated ? CGSize(width: natural.height, height: natural.width) : natural
    guard upright.width >= 1, upright.height >= 1 else {
        throw TranscribeFailure(code: "invalid", message: "\(camera.lastPathComponent) has an empty video track")
    }
    // The reader scales (in file orientation) so that the upright width is <= 640 px.
    let scale = min(1.0, analyzeMaxWidth / Double(upright.width))
    let outW = max(2, Int((Double(natural.width) * scale).rounded()))
    let outH = max(2, Int((Double(natural.height) * scale).rounded()))

    let reader: AVAssetReader
    do { reader = try AVAssetReader(asset: asset) } catch {
        throw TranscribeFailure(code: "invalid", message: "could not read \(camera.lastPathComponent): \(error.localizedDescription)")
    }
    let output = AVAssetReaderTrackOutput(track: track, outputSettings: [
        kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        kCVPixelBufferWidthKey as String: outW,
        kCVPixelBufferHeightKey as String: outH,
    ])
    output.alwaysCopiesSampleData = false
    guard reader.canAdd(output) else {
        throw TranscribeFailure(code: "invalid", message: "\(camera.lastPathComponent) cannot be decoded")
    }
    reader.add(output)
    guard reader.startReading() else {
        throw TranscribeFailure(code: "invalid", message: "could not decode \(camera.lastPathComponent): \(reader.error?.localizedDescription ?? "unknown error")")
    }
    let origin = track.timeRange.start.seconds.isFinite ? track.timeRange.start.seconds : 0

    job.progress(0, total)
    var samples: [FaceSample] = []
    var frames = 0
    var nextT = 0.0  // next sampling instant, camera file time
    while true {
        if job.isCancelled {
            reader.cancelReading()
            throw TranscribeFailure(code: "cancelled", message: "Face tracking cancelled")
        }
        job.beat()
        var stop = false
        try autoreleasepool {
            guard let sample = output.copyNextSampleBuffer() else { stop = true; return }
            let t = CMSampleBufferGetPresentationTimeStamp(sample).seconds - origin
            guard t.isFinite, t >= nextT - 1e-6, let pixels = CMSampleBufferGetImageBuffer(sample) else { return }
            // Next instant strictly after this frame, on the sampling grid (skips grid points a gap jumped over).
            nextT = (floor(t * fps + 1e-6) + 1) / fps
            frames += 1
            let request = VNDetectFaceRectanglesRequest()
            let handler = VNImageRequestHandler(cvPixelBuffer: pixels, orientation: orientation, options: [:])
            do { try handler.perform([request]) } catch {
                throw TranscribeFailure(code: "internal", message: "face detection failed: \(error.localizedDescription)")
            }
            let best = (request.results ?? []).max { $0.boundingBox.width * $0.boundingBox.height < $1.boundingBox.width * $1.boundingBox.height }
            if let face = best {
                let b = face.boundingBox  // normalised, origin bottom-left
                let x = min(max(Double(b.minX), 0), 1)
                let w = min(max(Double(b.width), 0), 1 - x)
                let yTop = min(max(1 - Double(b.maxY), 0), 1)
                let h = min(max(Double(b.height), 0), 1 - yTop)
                samples.append(FaceSample(t: round4(max(t, 0)), x: round4(x), y: round4(yTop), w: round4(w), h: round4(h),
                                          confidence: round4(Double(face.confidence))))
            }
            job.progress(min(max(t, 0), total), total)
        }
        if stop { break }
    }
    if reader.status == .failed {
        throw TranscribeFailure(code: "internal", message: "decoding \(camera.lastPathComponent) failed: \(reader.error?.localizedDescription ?? "unknown error")")
    }
    job.progress(total, total)
    return FaceAnalysis(width: Int(upright.width.rounded()), height: Int(upright.height.rounded()), frames: frames, samples: samples)
}
