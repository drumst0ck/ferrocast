import AVFoundation
import CoreMedia
import Foundation
import ScreenCaptureKit

// Audio recording (P4).
//
// Microphone: `AVCaptureSession` + `AVCaptureAudioDataOutput`, not `AVAudioEngine`. Its sample
// buffers carry presentation time stamps from the session's synchronization clock, which converts
// exactly to the host clock (the clock of the screen writer), and they are the same `CMSampleBuffer`
// type ScreenCaptureKit delivers for system audio, so mic and system audio share one writer. An
// `AVAudioEngine` tap would only give `AVAudioTime` host ticks for the first buffer of a render
// cycle and needs a format conversion to reach the writer.
//
// System audio: the audio output of the SAME `SCStream` as the screen (`capturesAudio`, the app's own
// sounds excluded); `Recorder` hands those buffers to `appendSystemAudio`.

final class MicCapture: NSObject, AVCaptureAudioDataOutputSampleBufferDelegate {
    let device: AVCaptureDevice
    let track: TrackWriter
    private let session = AVCaptureSession()
    private let output = AVCaptureAudioDataOutput()
    private let queue = DispatchQueue(label: "dev.ferrocast.capture.mic")
    private var armedAt: Double?
    private var lostReported = false
    private let lock = NSLock()

    init(device: AVCaptureDevice, final: URL) throws {
        self.device = device
        self.track = TrackWriter(final: final, kind: .audio)
        super.init()
        session.beginConfiguration()
        let input = try AVCaptureDeviceInput(device: device)
        guard session.canAddInput(input) else {
            session.commitConfiguration()
            throw RecordFailure(message: "cannot use the microphone \(device.localizedName)")
        }
        session.addInput(input)
        output.setSampleBufferDelegate(self, queue: queue)
        guard session.canAddOutput(output) else {
            session.commitConfiguration()
            throw RecordFailure(message: "cannot read audio from the microphone")
        }
        session.addOutput(output)
        session.commitConfiguration()
    }

    func start() throws {
        session.startRunning()
        guard session.isRunning else { throw RecordFailure(message: "the microphone did not start") }
    }

    func stop() {
        if session.isRunning { session.stopRunning() }
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sb: CMSampleBuffer, from connection: AVCaptureConnection) {
        let host = hostSeconds(of: CMSampleBufferGetPresentationTimeStamp(sb), clock: session.synchronizationClock)
        track.append(sb, host: host)
    }

    func checkLost(now: Double) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if armedAt == nil { armedAt = now }
        guard !lostReported, !track.isLost else { return false }
        let last = max(track.sampleHostLast, armedAt ?? now)
        if !device.isConnected || now - last > 3 || !session.isRunning {
            lostReported = true
            track.markLost(at: now)
            stop()
            return true
        }
        return false
    }
}

/// System audio buffer from the screen stream; SCK stamps it with the host clock.
func appendSystemAudio(_ track: TrackWriter, _ sb: CMSampleBuffer) {
    guard CMSampleBufferIsValid(sb), CMSampleBufferDataIsReady(sb) else { return }
    track.append(sb, host: CMSampleBufferGetPresentationTimeStamp(sb).seconds)
}

/// The device a request names (`{"device": id}`), or the system default; `nil` + a reason when none.
func pickDevice(_ kind: MediaKind, _ choice: [String: Any]) -> (AVCaptureDevice?, String) {
    if let wanted = choice["device"] as? String, !wanted.isEmpty {
        if let d = AVCaptureDevice(uniqueID: wanted), d.hasMediaType(kind.type) { return (d, "") }
        return (nil, "\(kind.label.lowercased()) \(wanted) not found")
    }
    if let d = AVCaptureDevice.default(for: kind.type) { return (d, "") }
    return (nil, "no \(kind.label.lowercased()) is available")
}
