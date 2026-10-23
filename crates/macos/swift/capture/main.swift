import CoreGraphics
import Foundation
import Metal
import ScreenCaptureKit

// ferrocast-capture: NDJSON requests on stdin, protocol lines on stdout, logs on stderr.
// No text or paths in argv. Exits on stdin EOF.

setvbuf(stdout, nil, _IOLBF, 0)
// Initialise CoreGraphics before any ScreenCaptureKit call: `SCContentFilter(desktopIndependentWindow:)` in a
// process that never touched CoreGraphics aborts with `CGS_REQUIRE_INIT` (seen live: Capture Window saved
// nothing and the helper logged the assertion). Window recording uses the same filter.
_ = CGMainDisplayID()
// A closed pipe must not kill the helper while it is finishing a recording.
signal(SIGPIPE, SIG_IGN)

// SIGTERM / SIGINT: finish an active recording (playable file), then exit. The main thread is
// blocked reading stdin, so the handlers run on a global queue.
var signalSources: [DispatchSourceSignal] = []
for sig in [SIGTERM, SIGINT] {
    signal(sig, SIG_IGN)
    let source = DispatchSource.makeSignalSource(signal: sig, queue: .global())
    source.setEventHandler {
        shutdownRecorder()
        shutdownExport()
        shutdownTranscribe()
        shutdownAnalyze()
        exit(0)
    }
    source.resume()
    signalSources.append(source)
}

func sckAvailable() -> Bool {
    if #available(macOS 14, *) { return true }
    return false
}

func handle(_ req: [String: Any]) {
    let id = req["id"]
    guard let cmd = req["cmd"] as? String else {
        fail(id, "invalid", "missing cmd")
        return
    }
    if cmd == "hello" {
        reply(id, ["protocol": protocolVersion, "os": osVersionString(), "version": helperVersion,
                   "sck": sckAvailable(), "metal": MTLCreateSystemDefaultDevice() != nil,
                   "capabilities": sckAvailable() ? ["shareable", "screenshot", "record", "render.frame", "render", "devices", "record.sources", "transcribe", "analyze.face"] : []])
        return
    }
    guard #available(macOS 14, *) else {
        fail(id, "unsupported-os", "Screen capture needs macOS 14")
        return
    }
    switch cmd {
    case "shareable": handleShareable(id)
    case "devices": handleDevices(id)
    case "permissions": handlePermissions(id, req)
    case "screenshot": handleScreenshot(id, req)
    case "record.start": handleRecordStart(id, req)
    case "record.stop", "record.pause", "record.resume": handleRecordControl(id, cmd)
    case "render.frame":
        if exportInProgress() { fail(id, "busy", "an export is running") } else { handleRenderFrame(id, req) }
    case "render": handleRender(id, req)
    case "cancel": if !handleTranscribeCancel(id, req) && !handleAnalyzeCancel(id, req) { handleCancel(id, req) }
    case "transcribe": handleTranscribe(id, req)
    case "analyze.face": handleAnalyzeFace(id, req)
    default: fail(id, "invalid", "unknown command: \(cmd)")
    }
}

while let line = readLine(strippingNewline: true) {
    if line.trimmingCharacters(in: .whitespaces).isEmpty { continue }
    guard let data = line.data(using: .utf8),
          let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        fail(nil, "invalid", "not a JSON object")
        continue
    }
    handle(obj)
}

// stdin closed: the app is gone. Keep a playable recording if one is running.
shutdownRecorder()
shutdownExport()
shutdownTranscribe()
shutdownAnalyze()
