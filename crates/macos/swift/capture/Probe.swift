import AVFoundation
import Foundation

// `probe`: duration and tracks of media files inside a project, read-only (recovery of a recording
// that never got its metadata, see `record_ctl::recover_project`).
//
// Request  {"id":N,"cmd":"probe","project":"/abs/Demo.ferrocast","files":["screen.mov","mic.m4a"]}
// Reply    {"id":N,"ok":true,"files":[{"file":"screen.mov","duration":12.5,"video":true,"audio":false,"size":[w,h]}]}
// A file that is missing, not a plain name, unreadable or has no duration is left out of the list.

func handleProbe(_ id: Any?, _ req: [String: Any]) {
    guard let project = req["project"] as? String, project.hasPrefix("/") else {
        fail(id, "invalid", "probe needs an absolute project path")
        return
    }
    guard let names = req["files"] as? [String], names.count <= 16 else {
        fail(id, "invalid", "probe needs a list of file names")
        return
    }
    var isDir: ObjCBool = false
    guard FileManager.default.fileExists(atPath: project, isDirectory: &isDir), isDir.boolValue else {
        fail(id, "not-found", "project folder not found")
        return
    }
    var out: [[String: Any]] = []
    for name in names {
        guard isPlainFileName(name) else {
            fail(id, "invalid", "file names must be plain names inside the project")
            return
        }
        let url = URL(fileURLWithPath: project).appendingPathComponent(name)
        guard FileManager.default.fileExists(atPath: url.path) else { continue }
        let asset = AVURLAsset(url: url)
        let probed: (Double, [Int], Bool, Bool)? = try? blocking {
            let d = try await asset.load(.duration).seconds
            let v = try await asset.loadTracks(withMediaType: .video)
            let a = try await asset.loadTracks(withMediaType: .audio)
            var size = [0, 0]
            if let t = v.first {
                let s = try await t.load(.naturalSize)
                size = [Int(abs(s.width).rounded()), Int(abs(s.height).rounded())]
            }
            return (d, size, !v.isEmpty, !a.isEmpty)
        }
        guard let (duration, size, video, audio) = probed, duration.isFinite, duration > 0, video || audio else { continue }
        out.append(["file": name, "duration": duration, "video": video, "audio": audio, "size": size])
    }
    reply(id, ["files": out])
}
