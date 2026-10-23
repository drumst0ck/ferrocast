import AVFoundation
import Foundation

// `devices` and `permissions` (P4).
//
// `devices` lists what is plugged in and NEVER prompts: `AVCaptureDevice.DiscoverySession` and
// `default(for:)` do not need a grant. Without one the system may hide device names; they are then
// reported as an empty string (the app shows a generic label).

private func positionName(_ p: AVCaptureDevice.Position) -> String {
    switch p {
    case .front: return "front"
    case .back: return "back"
    default: return "unspecified"
    }
}

@available(macOS 14, *)
func listCameras() -> [[String: Any]] {
    let types: [AVCaptureDevice.DeviceType] = [.builtInWideAngleCamera, .external, .continuityCamera, .deskViewCamera]
    let found = AVCaptureDevice.DiscoverySession(deviceTypes: types, mediaType: .video, position: .unspecified).devices
    let fallback = AVCaptureDevice.default(for: .video)?.uniqueID
    return found.map {
        ["id": $0.uniqueID, "name": $0.localizedName, "position": positionName($0.position), "default": $0.uniqueID == fallback]
    }
}

@available(macOS 14, *)
func listMicrophones() -> [[String: Any]] {
    let found = AVCaptureDevice.DiscoverySession(deviceTypes: [.microphone, .external], mediaType: .audio, position: .unspecified).devices
    let fallback = AVCaptureDevice.default(for: .audio)?.uniqueID
    return found.map { ["id": $0.uniqueID, "name": $0.localizedName, "default": $0.uniqueID == fallback] }
}

@available(macOS 14, *)
func handleDevices(_ id: Any?) {
    reply(id, ["cameras": listCameras(), "microphones": listMicrophones()])
}

/// `permissions`: report both states; kinds named in `request` that are `notDetermined` make the
/// system prompt (the only place the helper ever asks). Replies after the user answered.
func handlePermissions(_ id: Any?, _ req: [String: Any]) {
    let asked = (req["request"] as? [String] ?? []).compactMap { MediaKind(rawValue: $0) }
    for kind in asked where kind.access == "notDetermined" {
        let sem = DispatchSemaphore(value: 0)
        AVCaptureDevice.requestAccess(for: kind.type) { _ in sem.signal() }
        sem.wait()
    }
    reply(id, ["camera": MediaKind.camera.access, "microphone": MediaKind.microphone.access])
}
