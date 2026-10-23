import AVFoundation
import Foundation
#if canImport(Speech)
import Speech
#endif

// `transcribe` (P5): on-device speech to text for one audio file of a project.
//
// Request  {"id":N,"cmd":"transcribe","project":"/abs/Demo.ferrocast","audio":"mic.m4a","locale":"en-US"}
//   `audio` is a plain file name inside the project (no separator, no `..`; default `mic.m4a`),
//   `locale` a BCP-47 identifier (default: the system locale). Paths travel in JSON only.
// Events   {"event":"progress","id":N,"done_secs":D,"total_secs":T}  (at most ~4/s)
//          {"event":"heartbeat","id":N}                                (every ~10 s without any other line)
// Reply    {"id":N,"ok":true,"language":"en-US","words":42,"path":"/abs/Demo.ferrocast/transcript.json"}
// Errors   invalid | not-found | busy | cancelled | disk-full | internal, and
//          permission-speech        Speech Recognition is not authorized: replied BEFORE any work and the
//                                   helper exits with code 1 (like permission-screen). It NEVER calls
//                                   `requestAuthorization`: the app coordinates the system prompt.
//          transcription-unavailable  no ON-DEVICE recogniser for the locale. The helper never falls back to
//                                   the network and never downloads speech assets by itself.
// Output   <project>/transcript.json = {"language":"en-US","words":[{"text","start","end","confidence"}]},
//          seconds on the SOURCE audio timeline (the audio source's offset is applied by the Rust side).
//          Written to transcript.json.partial and renamed; failure and cancel leave neither file.
//
// Engines: macOS 26+ `SpeechAnalyzer` + `SpeechTranscriber` when the locale's assets are INSTALLED;
// otherwise `SFSpeechRecognizer` with `requiresOnDeviceRecognition = true` (macOS 14/15, and 26 for locales
// whose new assets are not installed). Audio is cut into ~50 s chunks (2 s of context each side, words are
// owned by the chunk containing their midpoint) for `SFSpeechRecognizer`: the documented 1-minute cap
// belongs to the server mode and was NOT verified for on-device on this Mac (no authorization), so the
// helper stays below it. The new engine takes the whole file.
// Threading: the work runs on its own thread; stdin (`cancel`) keeps being served. One job at a time.

private let transcribeLock = NSLock()
private var activeTranscribe: TranscribeJob?

final class TranscribeJob {
    let id: Any?
    let numericId: Int
    let finished = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var cancelRequested = false
    private var lastSend = DispatchTime.now()
    private var lastProgress = DispatchTime(uptimeNanoseconds: 0)

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

    /// Progress at most 4 times a second (the last one, done == total, always goes out).
    func progress(_ done: Double, _ total: Double) {
        lock.lock()
        let now = DispatchTime.now()
        let due = Double(now.uptimeNanoseconds &- lastProgress.uptimeNanoseconds) >= 250e6 || done >= total
        if due {
            lastProgress = now
            lastSend = now
        }
        lock.unlock()
        if due {
            send(["event": "progress", "id": id as Any, "done_secs": min(max(done, 0), total), "total_secs": total])
        }
    }

    /// Sign of life when nothing was sent for 10 s.
    func beat() {
        lock.lock()
        let quiet = Double(DispatchTime.now().uptimeNanoseconds &- lastSend.uptimeNanoseconds) >= 10e9
        if quiet { lastSend = DispatchTime.now() }
        lock.unlock()
        if quiet { send(["event": "heartbeat", "id": id as Any]) }
    }
}

struct TranscribeFailure: Error {
    let code: String
    let message: String
}

struct TranscribedWord {
    var text: String
    var start: Double
    var end: Double
    var confidence: Double
}

/// Plain file name inside the project directory: no separator, no `..`, no hidden or empty name.
func isPlainFileName(_ name: String) -> Bool {
    !name.isEmpty && !name.contains("/") && !name.contains("\\") && !name.contains("\0") && !name.hasPrefix(".") && !name.contains("..")
        && name.utf8.count <= 255
}

private func isLocaleIdentifier(_ s: String) -> Bool {
    !s.isEmpty && s.count <= 35 && s.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }
}

func bcp47(_ locale: Locale) -> String {
    locale.identifier(.bcp47)
}

/// "English (United States)" for the message the app shows; the identifier itself when unknown.
private func languageName(_ identifier: String) -> String {
    Locale(identifier: "en_US").localizedString(forIdentifier: identifier) ?? identifier
}

func unavailableMessage(_ identifier: String) -> String {
    // Same sentence as `TRANSCRIPTION_UNAVAILABLE_TEMPLATE` in crates/macos/src/capture.rs.
    "On-device transcription is not available for \(languageName(identifier)) on this Mac."
}

func handleTranscribe(_ id: Any?, _ req: [String: Any]) {
    guard let project = req["project"] as? String, project.hasPrefix("/") else {
        fail(id, "invalid", "transcribe needs an absolute project path")
        return
    }
    let audioName = (req["audio"] as? String) ?? "mic.m4a"
    guard isPlainFileName(audioName) else {
        fail(id, "invalid", "audio must be a plain file name inside the project")
        return
    }
    var localeId = bcp47(Locale.current)
    if let l = req["locale"] as? String {
        guard isLocaleIdentifier(l) else {
            fail(id, "invalid", "locale must be a BCP-47 identifier such as en-US")
            return
        }
        localeId = l
    }
    var isDir: ObjCBool = false
    guard FileManager.default.fileExists(atPath: project, isDirectory: &isDir), isDir.boolValue else {
        fail(id, "not-found", "project folder not found")
        return
    }
    let audioURL = URL(fileURLWithPath: project).appendingPathComponent(audioName)
    guard FileManager.default.fileExists(atPath: audioURL.path) else {
        fail(id, "not-found", "\(audioName) is not in the project")
        return
    }
    // Permission BEFORE any work, and never the system prompt: the app asks the user first.
    guard SFSpeechRecognizer.authorizationStatus() == .authorized else {
        fail(id, "permission-speech", "Speech Recognition is not authorized")
        exit(1)
    }
    let job = TranscribeJob(id: id)
    transcribeLock.lock()
    if activeTranscribe != nil {
        transcribeLock.unlock()
        fail(id, "busy", "a transcription is already running")
        return
    }
    activeTranscribe = job
    transcribeLock.unlock()

    let thread = Thread {
        defer {
            transcribeLock.lock()
            if activeTranscribe === job { activeTranscribe = nil }
            transcribeLock.unlock()
            job.finished.signal()
        }
        let out = URL(fileURLWithPath: project).appendingPathComponent("transcript.json")
        let partial = URL(fileURLWithPath: project).appendingPathComponent("transcript.json.partial")
        do {
            let (language, words) = try runTranscription(job: job, audio: audioURL, localeId: localeId)
            if job.isCancelled { throw TranscribeFailure(code: "cancelled", message: "Transcription cancelled") }
            let body: [String: Any] = [
                "language": language,
                "words": words.map { ["text": $0.text, "start": $0.start, "end": $0.end, "confidence": $0.confidence] as [String: Any] },
            ]
            do {
                let data = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])
                try data.write(to: partial)
                guard rename(partial.path, out.path) == 0 else {
                    throw TranscribeFailure(code: "internal", message: "could not write transcript.json: \(String(cString: strerror(errno)))")
                }
            } catch let e as TranscribeFailure {
                try? FileManager.default.removeItem(at: partial)
                throw e
            } catch {
                try? FileManager.default.removeItem(at: partial)
                let full = (error as NSError).code == NSFileWriteOutOfSpaceError
                throw TranscribeFailure(code: full ? "disk-full" : "internal", message: "could not write transcript.json: \(error.localizedDescription)")
            }
            reply(id, ["language": language, "words": words.count, "path": out.path])
        } catch let e as TranscribeFailure {
            try? FileManager.default.removeItem(at: partial)
            fail(id, e.code, e.message)
        } catch {
            try? FileManager.default.removeItem(at: partial)
            fail(id, "internal", "transcription failed: \(error.localizedDescription)")
        }
    }
    thread.qualityOfService = .userInitiated
    thread.start()
}

/// `cancel` for a transcription: true when `target` is the running one (the reply was sent).
func handleTranscribeCancel(_ id: Any?, _ req: [String: Any]) -> Bool {
    let target = (req["target"] as? NSNumber)?.intValue
    transcribeLock.lock()
    let job = activeTranscribe
    transcribeLock.unlock()
    guard let job, job.numericId == target else { return false }
    job.cancel()
    reply(id, [:])
    return true
}

/// Stdin closed or SIGTERM: stop and wait (bounded) for the thread to remove its partial file.
func shutdownTranscribe() {
    transcribeLock.lock()
    let job = activeTranscribe
    transcribeLock.unlock()
    guard let job else { return }
    job.cancel()
    _ = job.finished.wait(timeout: .now() + 10)
}

// MARK: - Engines

/// Picks the engine and returns (language identifier actually used, words on the source timeline).
private func runTranscription(job: TranscribeJob, audio: URL, localeId: String) throws -> (String, [TranscribedWord]) {
    let asset = AVURLAsset(url: audio)
    let total = asset.duration.seconds
    guard total.isFinite, total > 0, !asset.tracks(withMediaType: .audio).isEmpty else {
        throw TranscribeFailure(code: "invalid", message: "\(audio.lastPathComponent) has no audio to transcribe")
    }
    job.progress(0, total)

    #if compiler(>=6.2) && canImport(Speech)
    if #available(macOS 26, *), NSClassFromString("Speech.SpeechAnalyzer") != nil {
        if let words = try transcribeWithSpeechAnalyzer(job: job, audio: audio, localeId: localeId, total: total) {
            job.progress(total, total)
            return (localeId, words)
        }
    }
    #endif

    let words = try transcribeWithRecognizer(job: job, asset: asset, localeId: localeId, total: total)
    job.progress(total, total)
    return (localeId, words)
}

/// Waits for `done`, serving cancel/heartbeat from this thread; `onCancel` runs once when cancelled.
private func wait(_ done: DispatchSemaphore, job: TranscribeJob, onCancel: () -> Void) {
    var cancelled = false
    while done.wait(timeout: .now() + 0.25) == .timedOut {
        if job.isCancelled && !cancelled {
            cancelled = true
            onCancel()
        }
        job.beat()
    }
}

// SFSpeechRecognizer (macOS 14/15 and the fallback on 26) --------------------------------------

/// Length of audio per recognition request and the context added on each side.
private let chunkSeconds = 50.0
private let chunkContext = 2.0

private func transcribeWithRecognizer(job: TranscribeJob, asset: AVURLAsset, localeId: String, total: Double) throws -> [TranscribedWord] {
    guard let recognizer = SFSpeechRecognizer(locale: Locale(identifier: localeId)),
          recognizer.isAvailable, recognizer.supportsOnDeviceRecognition else {
        throw TranscribeFailure(code: "transcription-unavailable", message: unavailableMessage(localeId))
    }
    // The handlers must not run on the main queue: the main thread is blocked reading stdin.
    let queue = OperationQueue()
    queue.maxConcurrentOperationCount = 1
    recognizer.queue = queue

    var words: [TranscribedWord] = []
    var start = 0.0
    while start < total {
        if job.isCancelled { throw TranscribeFailure(code: "cancelled", message: "Transcription cancelled") }
        let own = start..<min(start + chunkSeconds, total)
        let from = max(0, own.lowerBound - chunkContext)
        let to = min(total, own.upperBound + chunkContext)
        let chunk = try exportAudio(asset: asset, from: from, to: to, job: job)
        defer { try? FileManager.default.removeItem(at: chunk) }
        let segments = try recognize(chunk, with: recognizer, job: job)
        let isLast = own.upperBound >= total
        for s in segments {
            let text = s.text.trimmingCharacters(in: .whitespacesAndNewlines)
            if text.isEmpty { continue }
            let a = from + s.start
            let b = from + s.start + s.duration
            let mid = (a + b) / 2
            // A word belongs to the chunk that contains its midpoint.
            if mid >= own.lowerBound && (mid < own.upperBound || isLast) {
                words.append(TranscribedWord(text: text, start: a, end: max(b, a), confidence: s.confidence))
            }
        }
        start = own.upperBound
        job.progress(start, total)
    }
    return words
}

private struct Segment {
    var text: String
    var start: Double
    var duration: Double
    var confidence: Double
}

private func recognize(_ url: URL, with recognizer: SFSpeechRecognizer, job: TranscribeJob) throws -> [Segment] {
    let request = SFSpeechURLRecognitionRequest(url: url)
    request.requiresOnDeviceRecognition = true
    request.shouldReportPartialResults = false
    request.taskHint = .dictation
    let done = DispatchSemaphore(value: 0)
    let box = NSLock()
    var segments: [Segment] = []
    var failure: Error?
    var finished = false
    let task = recognizer.recognitionTask(with: request) { result, error in
        box.lock()
        defer { box.unlock() }
        if finished { return }
        if let result {
            if result.isFinal {
                segments = result.bestTranscription.segments.map {
                    Segment(text: $0.substring, start: $0.timestamp, duration: $0.duration, confidence: Double($0.confidence))
                }
                finished = true
                done.signal()
            }
        } else if let error {
            // "No speech detected" (kAFAssistantErrorDomain 1110) is an empty chunk, not a failure.
            let e = error as NSError
            if !(e.domain == "kAFAssistantErrorDomain" && e.code == 1110) { failure = error }
            finished = true
            done.signal()
        }
    }
    wait(done, job: job) { task.cancel() }
    if job.isCancelled { throw TranscribeFailure(code: "cancelled", message: "Transcription cancelled") }
    box.lock()
    defer { box.unlock() }
    if let failure { throw TranscribeFailure(code: "internal", message: "speech recognition failed: \(failure.localizedDescription)") }
    return segments
}

/// Writes the audio of `asset` between `from` and `to` to a temporary .m4a (any container: screen.mov too).
private func exportAudio(asset: AVURLAsset, from: Double, to: Double, job: TranscribeJob) throws -> URL {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("ferrocast-transcribe-\(UUID().uuidString).m4a")
    guard let session = AVAssetExportSession(asset: asset, presetName: AVAssetExportPresetAppleM4A) else {
        throw TranscribeFailure(code: "internal", message: "could not prepare the audio for transcription")
    }
    session.outputURL = url
    session.outputFileType = .m4a
    session.timeRange = CMTimeRange(start: CMTime(seconds: from, preferredTimescale: 600), end: CMTime(seconds: to, preferredTimescale: 600))
    let done = DispatchSemaphore(value: 0)
    session.exportAsynchronously { done.signal() }
    wait(done, job: job) { session.cancelExport() }
    if job.isCancelled {
        try? FileManager.default.removeItem(at: url)
        throw TranscribeFailure(code: "cancelled", message: "Transcription cancelled")
    }
    guard session.status == .completed else {
        try? FileManager.default.removeItem(at: url)
        throw TranscribeFailure(code: "internal", message: "could not extract the audio: \(session.error?.localizedDescription ?? "unknown error")")
    }
    return url
}

// SpeechAnalyzer (macOS 26+) ---------------------------------------------------------------------
// Compiled only with a Swift 6.2+ compiler (Xcode 26 and later, whose SDK has the Speech types). A runtime
// `NSClassFromString("Speech.SpeechAnalyzer")` check guards the call; `#available(macOS 26, *)` makes the
// compiler weak-link the symbols. Returns nil when the new engine cannot serve the locale ON DEVICE (the
// caller falls back to SFSpeechRecognizer); it never downloads assets.
#if compiler(>=6.2) && canImport(Speech)
@available(macOS 26, *)
private func transcribeWithSpeechAnalyzer(job: TranscribeJob, audio: URL, localeId: String, total: Double) throws -> [TranscribedWord]? {
    final class Outcome: @unchecked Sendable {
        var words: [TranscribedWord]?
        var failure: TranscribeFailure?
        var analyzer: SpeechAnalyzer?
    }
    let outcome = Outcome()
    let done = DispatchSemaphore(value: 0)
    // SpeechAnalyzer reads an AVAudioFile: a .m4a goes straight in, anything else (screen.mov) is extracted first.
    var source = audio
    var temp: URL?
    if audio.pathExtension.lowercased() != "m4a" {
        let t = try exportAudio(asset: AVURLAsset(url: audio), from: 0, to: total, job: job)
        temp = t
        source = t
    }
    defer { if let temp { try? FileManager.default.removeItem(at: temp) } }

    Task.detached {
        defer { done.signal() }
        do {
            let wanted = Locale(identifier: localeId)
            guard SpeechTranscriber.isAvailable,
                  let supported = await SpeechTranscriber.supportedLocale(equivalentTo: wanted) else { return }
            let installed = await SpeechTranscriber.installedLocales.map { bcp47($0) }
            guard installed.contains(bcp47(supported)) else { return }
            let transcriber = SpeechTranscriber(
                locale: supported, transcriptionOptions: [], reportingOptions: [],
                attributeOptions: [.audioTimeRange, .transcriptionConfidence])
            let analyzer = SpeechAnalyzer(modules: [transcriber])
            outcome.analyzer = analyzer
            let file = try AVAudioFile(forReading: source)
            let collector = Task { () throws -> [TranscribedWord] in
                var words: [TranscribedWord] = []
                for try await result in transcriber.results {
                    for run in result.text.runs {
                        let text = String(result.text[run.range].characters).trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !text.isEmpty, let range = run[AttributeScopes.SpeechAttributes.TimeRangeAttribute.self] else { continue }
                        let confidence = run[AttributeScopes.SpeechAttributes.ConfidenceAttribute.self] ?? 1
                        words.append(TranscribedWord(text: text, start: range.start.seconds, end: range.end.seconds, confidence: confidence))
                    }
                    job.progress(result.range.end.seconds, total)
                }
                return words
            }
            if let last = try await analyzer.analyzeSequence(from: file) {
                try await analyzer.finalizeAndFinish(through: last)
            } else {
                await analyzer.cancelAndFinishNow()
            }
            outcome.words = try await collector.value
        } catch {
            outcome.failure = TranscribeFailure(code: "internal", message: "speech analysis failed: \(error.localizedDescription)")
        }
    }
    wait(done, job: job) {
        if let analyzer = outcome.analyzer { Task { await analyzer.cancelAndFinishNow() } }
    }
    if job.isCancelled { throw TranscribeFailure(code: "cancelled", message: "Transcription cancelled") }
    if let failure = outcome.failure { throw failure }
    return outcome.words
}
#endif
