// permcheck screen|ax|input|camera|mic -> granted | denied (| notDetermined | restricted). Never prompts.
import ApplicationServices
import AVFoundation
import CoreGraphics
import Foundation

func av(_ t: AVMediaType) -> String {
    switch AVCaptureDevice.authorizationStatus(for: t) {
    case .authorized: return "granted"
    case .denied: return "denied"
    case .restricted: return "restricted"
    default: return "notDetermined"
    }
}
func yn(_ b: Bool) -> String { b ? "granted" : "denied" }
switch CommandLine.arguments.dropFirst().first ?? "" {
case "screen": print(yn(CGPreflightScreenCaptureAccess()))
case "ax": print(yn(AXIsProcessTrusted()))
case "input": print(yn(CGPreflightListenEventAccess()))
case "camera": print(av(.video))
case "mic": print(av(.audio))
default: FileHandle.standardError.write(Data("usage: permcheck screen|ax|input|camera|mic\n".utf8)); exit(2)
}
