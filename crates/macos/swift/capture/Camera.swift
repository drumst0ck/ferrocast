import AVFoundation
import CoreMedia
import Foundation

// Camera recording (P4): AVCaptureSession -> TrackWriter (`camera.mov`).
//
// - 1280x720 (or the best preset below 1080p the device offers), 30 fps when the format allows it.
// - HEVC (H.264 fallback) like the screen take, written through the shared `TrackWriter`.
// - NOT mirrored here (`isVideoMirrored = false`): the renderer mirrors a selfie camera.
// - Sample time stamps are converted from the session clock to the host clock, so the offset to the
//   screen take's t0 is measured on the clock the screen writer uses.
// - The session is started BEFORE the screen stream and the writer is armed with the screen's t0
//   afterwards, so the camera is already warm when t0 is set; earlier frames are ignored.

final class CameraCapture: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    let device: AVCaptureDevice
    let track: TrackWriter
    private let session = AVCaptureSession()
    private let output = AVCaptureVideoDataOutput()
    private let queue = DispatchQueue(label: "dev.ferrocast.capture.camera")
    private var armedAt: Double?
    private var lostReported = false
    private let lock = NSLock()

    init(device: AVCaptureDevice, final: URL, fps: Int = 30) throws {
        self.device = device
        self.track = TrackWriter(final: final, kind: .video(fps: fps))
        super.init()
        session.beginConfiguration()
        for preset in [AVCaptureSession.Preset.hd1280x720, .iFrame960x540, .vga640x480, .medium] where device.supportsSessionPreset(preset) {
            session.sessionPreset = preset
            break
        }
        let input = try AVCaptureDeviceInput(device: device)
        guard session.canAddInput(input) else {
            session.commitConfiguration()
            throw RecordFailure(message: "cannot use the camera \(device.localizedName)")
        }
        session.addInput(input)
        output.alwaysDiscardsLateVideoFrames = true
        output.setSampleBufferDelegate(self, queue: queue)
        guard session.canAddOutput(output) else {
            session.commitConfiguration()
            throw RecordFailure(message: "cannot read frames from the camera")
        }
        session.addOutput(output)
        if let c = output.connection(with: .video), c.isVideoMirroringSupported {
            c.automaticallyAdjustsVideoMirroring = false
            c.isVideoMirrored = false
        }
        session.commitConfiguration()
        let want = Double(fps)
        if device.activeFormat.videoSupportedFrameRateRanges.contains(where: { $0.minFrameRate <= want && want <= $0.maxFrameRate }),
           (try? device.lockForConfiguration()) != nil {
            device.activeVideoMinFrameDuration = CMTime(value: 1, timescale: CMTimeScale(fps))
            device.activeVideoMaxFrameDuration = CMTime(value: 1, timescale: CMTimeScale(fps))
            device.unlockForConfiguration()
        }
    }

    /// Blocks until the session runs (or fails).
    func start() throws {
        session.startRunning()
        guard session.isRunning else { throw RecordFailure(message: "the camera did not start") }
    }

    func stop() {
        if session.isRunning { session.stopRunning() }
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sb: CMSampleBuffer, from connection: AVCaptureConnection) {
        let host = hostSeconds(of: CMSampleBufferGetPresentationTimeStamp(sb), clock: session.synchronizationClock)
        track.append(sb, host: host)
    }

    /// Unplugged, or silent for 3 s while recording: true once, at the moment it is detected.
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
