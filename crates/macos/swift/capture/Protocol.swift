import Foundation

// NDJSON protocol: one JSON object per line on stdin/stdout, logs on stderr only.

let helperVersion = "0.1.0"
let protocolVersion = 1

func log(_ message: String) {
    FileHandle.standardError.write(Data((message + "\n").utf8))
}

/// Write one protocol line to stdout.
func send(_ object: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]) else { return }
    var line = data
    line.append(0x0A)
    FileHandle.standardOutput.write(line)
}

func reply(_ id: Any?, _ fields: [String: Any]) {
    var o = fields
    if let id = id { o["id"] = id }
    o["ok"] = true
    send(o)
}

func fail(_ id: Any?, _ code: String, _ message: String) {
    var o: [String: Any] = ["error": code, "message": message]
    if let id = id { o["id"] = id }
    send(o)
}

func osVersionString() -> String {
    let v = ProcessInfo.processInfo.operatingSystemVersion
    return "\(v.majorVersion).\(v.minorVersion).\(v.patchVersion)"
}

func osAtLeast(_ major: Int) -> Bool {
    ProcessInfo.processInfo.operatingSystemVersion.majorVersion >= major
}
