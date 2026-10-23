// permask screen|ax|input|camera|mic: requests the permission and triggers the system prompt.
import ApplicationServices
import AVFoundation
import CoreGraphics
import Foundation

func ask(_ t: AVMediaType) {
    let sem = DispatchSemaphore(value: 0)
    AVCaptureDevice.requestAccess(for: t) { ok in print(ok ? "granted" : "denied"); sem.signal() }
    sem.wait()
}
switch CommandLine.arguments.dropFirst().first ?? "" {
case "screen": print(CGRequestScreenCaptureAccess() ? "granted" : "requested")
case "ax":
    let opts = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
    print(AXIsProcessTrustedWithOptions(opts) ? "granted" : "requested")
case "input": print(CGRequestListenEventAccess() ? "granted" : "requested")
case "camera": ask(.video)
case "mic": ask(.audio)
default: FileHandle.standardError.write(Data("usage: permask screen|ax|input|camera|mic\n".utf8)); exit(2)
}
