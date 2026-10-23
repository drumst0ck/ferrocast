import AppKit
import CoreGraphics
import Foundation

// Input event recorder -> `events.json` (format: crates/core/src/capture/events.rs, version 1).
//
// Started and stopped with the recording (Recorder.swift calls start/pause/resume/finish).
// Times are seconds on the recording clock of `screen.mov` (host clock, paused time removed).
// Positions are points relative to the capture display's top-left corner.
//
// Permissions, as observed on macOS 26 without any grant (see odd notes in the T17 handoff):
//   - cursor position:  CGEvent(source: nil).location, no permission.
//   - mouse buttons and scroll: listen-only CGEventTap on mouse types is created without Input
//     Monitoring. (NSEvent global monitors were not used: the helper's main thread is blocked in
//     readLine, so a monitor would never be delivered.)
//   - keys: a listen-only key tap needs Input Monitoring; without it `CGEvent.tapCreate` returns nil.
//     Then everything else is recorded and the recorder reports `input-monitoring-off`.
//   - keys are stored only as shortcuts (a non-shift modifier). Plain typing is never stored.

// MARK: pure log (no AppKit, no clocks: driven by the owner, unit-testable)

struct EventsScreen {
    var size: (Int, Int)
    var scale: Double
    var displayFrame: CGRect
    /// The captured area inside the display (rect/window targets); written as an extra field.
    var captureRect: CGRect?
}

final class EventLog {
    private struct Cursor { var t: Double; var x: Double; var y: Double; var kind: String }
    private struct Click { var t: Double; var x: Double; var y: Double; var button: String; var down: Bool }
    private struct Key { var t: Double; var down: Bool; var mods: [String]; var code: String?; var ch: String? }
    private struct Scroll { var t: Double; var dx: Double; var dy: Double }
    private struct Win { var t: Double; var id: UInt64; var frame: CGRect; var app: String }

    let screen: EventsScreen
    // Append-only buffers.
    private var cursor: [Cursor] = []
    private var clicks: [Click] = []
    private var keys: [Key] = []
    private var scrolls: [Scroll] = []
    private var windows: [Win] = []
    /// A repeated identical cursor sample is held back (only the latest one) so an idle pointer
    /// costs nothing, yet the moment it started to rest and the moment it left are both exact.
    private var held: Cursor?

    init(screen: EventsScreen) { self.screen = screen }

    var counts: (cursor: Int, clicks: Int, keys: Int, scroll: Int, windows: Int) {
        (cursor.count + (held == nil ? 0 : 1), clicks.count, keys.count, scrolls.count, windows.count)
    }

    func addCursor(t: Double, x: Double, y: Double, kind: String) {
        let s = Cursor(t: t, x: x, y: y, kind: kind)
        if let last = cursor.last, last.x == x, last.y == y, last.kind == kind {
            held = s
            return
        }
        if let h = held { cursor.append(h); held = nil }
        cursor.append(s)
    }

    func addClick(t: Double, x: Double, y: Double, button: String, down: Bool) {
        clicks.append(Click(t: t, x: x, y: y, button: button, down: down))
    }

    func addScroll(t: Double, dx: Double, dy: Double) {
        if dx == 0 && dy == 0 { return }
        scrolls.append(Scroll(t: t, dx: dx, dy: dy))
    }

    func addWindow(t: Double, id: UInt64, frame: CGRect, app: String) {
        windows.append(Win(t: t, id: id, frame: frame, app: app))
    }

    /// Privacy rule, applied at the source: only a key pressed with command, control or option is
    /// stored (shortcut). Shift alone, caps lock, fn and bare keys store nothing. Returns whether
    /// the key was stored.
    @discardableResult
    func addKey(t: Double, down: Bool, mods: [String], code: String?, char: String?) -> Bool {
        let shortcut = mods.filter { $0 == "cmd" || $0 == "ctrl" || $0 == "alt" }
        guard !shortcut.isEmpty else { return false }
        let all = ["cmd", "ctrl", "alt", "shift"].filter { mods.contains($0) }
        keys.append(Key(t: t, down: down, mods: all, code: code, ch: char))
        return true
    }

    // MARK: encoding

    private static func num(_ v: Double, _ places: Int) -> String {
        guard v.isFinite else { return "0" }
        var s = String(format: "%.\(places)f", v)
        if s.contains(".") {
            while s.hasSuffix("0") { s.removeLast() }
            if s.hasSuffix(".") { s.removeLast() }
        }
        return s == "-0" ? "0" : s
    }

    private static func str(_ s: String) -> String {
        guard let d = try? JSONSerialization.data(withJSONObject: s, options: [.fragmentsAllowed]),
              let out = String(data: d, encoding: .utf8) else { return "\"\"" }
        return out
    }

    private static func rect(_ r: CGRect) -> String {
        "[" + [r.origin.x, r.origin.y, r.size.width, r.size.height].map { num($0, 2) }.joined(separator: ",") + "]"
    }

    func json() -> Data {
        var out = "{\"version\":1,\"screen\":{\"size\":[\(screen.size.0),\(screen.size.1)],\"scale\":\(Self.num(screen.scale, 3))"
        out += ",\"display_frame\":\(Self.rect(screen.displayFrame))"
        if let r = screen.captureRect { out += ",\"capture_rect\":\(Self.rect(r))" }
        out += "},\"cursor\":["
        var all = cursor
        if let h = held { all.append(h) }
        out += all.map { "[\(Self.num($0.t, 4)),\(Self.num($0.x, 2)),\(Self.num($0.y, 2)),\(Self.str($0.kind))]" }
            .joined(separator: ",")
        out += "],\"clicks\":["
        out += clicks.map {
            "{\"t\":\(Self.num($0.t, 4)),\"x\":\(Self.num($0.x, 2)),\"y\":\(Self.num($0.y, 2)),\"button\":\(Self.str($0.button)),\"\($0.down ? "down" : "up")\":true}"
        }.joined(separator: ",")
        out += "],\"keys\":["
        out += keys.map { k in
            var s = "{\"t\":\(Self.num(k.t, 4)),\"down\":\(k.down),\"mods\":[" + k.mods.map(Self.str).joined(separator: ",") + "]"
            if let c = k.code { s += ",\"code\":\(Self.str(c))" }
            if let c = k.ch { s += ",\"char\":\(Self.str(c))" }
            return s + "}"
        }.joined(separator: ",")
        out += "],\"scroll\":["
        out += scrolls.map { "{\"t\":\(Self.num($0.t, 4)),\"dx\":\(Self.num($0.dx, 2)),\"dy\":\(Self.num($0.dy, 2))}" }
            .joined(separator: ",")
        out += "],\"windows\":["
        out += windows.map {
            "{\"t\":\(Self.num($0.t, 4)),\"id\":\($0.id),\"frame\":\(Self.rect($0.frame)),\"app\":\(Self.str($0.app))}"
        }.joined(separator: ",")
        out += "]}\n"
        return Data(out.utf8)
    }

    /// Temp file next to the target, then rename(2): a reader never sees a half-written file.
    func write(to url: URL) throws {
        let tmp = url.path + ".partial"
        try json().write(to: URL(fileURLWithPath: tmp))
        if rename(tmp, url.path) != 0 {
            let err = String(cString: strerror(errno))
            try? FileManager.default.removeItem(atPath: tmp)
            throw RecordFailure(message: "could not write events.json: \(err)")
        }
    }
}

// MARK: key naming

private let keyCodes: [Int: (code: String, char: String?)] = [
    0: ("KeyA", "a"), 1: ("KeyS", "s"), 2: ("KeyD", "d"), 3: ("KeyF", "f"), 4: ("KeyH", "h"), 5: ("KeyG", "g"),
    6: ("KeyZ", "z"), 7: ("KeyX", "x"), 8: ("KeyC", "c"), 9: ("KeyV", "v"), 11: ("KeyB", "b"), 12: ("KeyQ", "q"),
    13: ("KeyW", "w"), 14: ("KeyE", "e"), 15: ("KeyR", "r"), 16: ("KeyY", "y"), 17: ("KeyT", "t"),
    18: ("Digit1", "1"), 19: ("Digit2", "2"), 20: ("Digit3", "3"), 21: ("Digit4", "4"), 22: ("Digit6", "6"),
    23: ("Digit5", "5"), 24: ("Equal", "="), 25: ("Digit9", "9"), 26: ("Digit7", "7"), 27: ("Minus", "-"),
    28: ("Digit8", "8"), 29: ("Digit0", "0"), 30: ("BracketRight", "]"), 31: ("KeyO", "o"), 32: ("KeyU", "u"),
    33: ("BracketLeft", "["), 34: ("KeyI", "i"), 35: ("KeyP", "p"), 36: ("Enter", nil), 37: ("KeyL", "l"),
    38: ("KeyJ", "j"), 39: ("Quote", "'"), 40: ("KeyK", "k"), 41: ("Semicolon", ";"), 42: ("Backslash", "\\"),
    43: ("Comma", ","), 44: ("Slash", "/"), 45: ("KeyN", "n"), 46: ("KeyM", "m"), 47: ("Period", "."),
    48: ("Tab", nil), 49: ("Space", nil), 50: ("Backquote", "`"), 51: ("Backspace", nil), 53: ("Escape", nil),
    96: ("F5", nil), 97: ("F6", nil), 98: ("F7", nil), 99: ("F3", nil), 100: ("F8", nil), 101: ("F9", nil),
    103: ("F11", nil), 109: ("F10", nil), 111: ("F12", nil), 118: ("F4", nil), 120: ("F2", nil), 122: ("F1", nil),
    123: ("ArrowLeft", nil), 124: ("ArrowRight", nil), 125: ("ArrowDown", nil), 126: ("ArrowUp", nil),
]

func keyName(_ keycode: Int) -> (code: String, char: String?) {
    keyCodes[keycode] ?? ("Key\(keycode)", nil)
}

// MARK: cursor kind

/// Cursor shapes known by (image size, hot spot). Spike result on macOS 26 (no grants): the
/// `NSCursor.currentSystem` image is available from a helper without a GUI session, but hashing it
/// is costly (the arrow's TIFF is ~580 KB) and `NSCursor.arrow` / `.iBeam` themselves return no
/// image there, so shapes are told apart by geometry. The table takes the system cursors that do
/// have an image, plus the measured arrow and I-beam geometry as fixed entries. Shapes that share a
/// geometry with an earlier entry (closed hand, not-allowed) are not distinguished. Unknown shapes,
/// and any failure, are `arrow`.
final class CursorKinds {
    private var table: [String: String] = ["28x40@5,5": "arrow", "23x22@12,11": "ibeam"]

    private static func sig(_ c: NSCursor) -> String {
        "\(Int(c.image.size.width))x\(Int(c.image.size.height))@\(Int(c.hotSpot.x)),\(Int(c.hotSpot.y))"
    }

    init() {
        let known: [(String, NSCursor)] = [
            ("hand", .pointingHand), ("crosshair", .crosshair), ("grab", .openHand),
            ("resize-h", .resizeLeftRight), ("resize-v", .resizeUpDown),
        ]
        for (name, cursor) in known where cursor.image.size.width > 0 && table[Self.sig(cursor)] == nil {
            table[Self.sig(cursor)] = name
        }
    }

    func current() -> String {
        guard let c = NSCursor.currentSystem, c.image.size.width > 0 else { return "arrow" }
        return table[Self.sig(c)] ?? "arrow"
    }
}

// MARK: live recorder

struct EventsConfig {
    var screen: EventsScreen
    /// Display the positions are relative to (CG global, top-left origin, points).
    var displayFrame: CGRect { screen.displayFrame }
    /// Set for a window target: only this window's frame is tracked.
    var windowID: UInt32?
    /// Windows never recorded (Ferrocast's own HUD).
    var excluded: Set<UInt32>
}

final class InputRecorder {
    let config: EventsConfig
    let log: EventLog
    let url: URL
    private let lock = NSLock()
    private var t0 = 0.0
    private var pausedTotal = 0.0
    private var pausedAt: Double?
    private var finished = false
    private var timer: DispatchSourceTimer?
    private let queue = DispatchQueue(label: "dev.ferrocast.capture.events", qos: .userInteractive)
    private var tapThread: Thread?
    private var tapRunLoop: CFRunLoop?
    private var taps: [CFMachPort] = []
    private var kinds: CursorKinds?
    private var tickCount = 0
    private var lastKind = "arrow"
    private var lastFrames: [UInt32: CGRect] = [:]
    /// False once started without Input Monitoring.
    private(set) var keysAvailable = false
    private(set) var mouseTapAvailable = false

    init(config: EventsConfig, url: URL) {
        self.config = config
        self.url = url
        self.log = EventLog(screen: config.screen)
    }

    /// Recording-clock seconds at host time `now`; nil while paused.
    private func rel(_ now: Double) -> Double? { pausedAt == nil ? max(0, now - t0 - pausedTotal) : nil }

    private func local(_ p: CGPoint) -> (Double, Double) {
        (Double(p.x - config.displayFrame.origin.x), Double(p.y - config.displayFrame.origin.y))
    }

    /// Begin at host time `at` (the recording's t = 0). Returns false when the key tap could not be
    /// created (Input Monitoring is off); everything else is recorded regardless.
    @discardableResult
    func start(at: Double) -> Bool {
        lock.lock()
        t0 = at
        kinds = CursorKinds()
        sampleLocked(now: at)
        pollWindowsLocked(now: at)
        lock.unlock()
        startTaps()
        let t = DispatchSource.makeTimerSource(queue: queue)
        t.schedule(deadline: .now() + .milliseconds(16), repeating: .nanoseconds(16_666_667), leeway: .milliseconds(1))
        t.setEventHandler { [weak self] in self?.tick() }
        timer = t
        t.resume()
        return keysAvailable
    }

    func pause(at now: Double) {
        lock.lock()
        if pausedAt == nil { pausedAt = now }
        lock.unlock()
    }

    func resume(at now: Double) {
        lock.lock()
        if let p = pausedAt { pausedTotal += max(0, now - p) }
        pausedAt = nil
        lock.unlock()
    }

    /// Stop every source. With `write`, take a last sample at host time `now` and write events.json.
    func finish(at now: Double, write: Bool) {
        lock.lock()
        if finished { lock.unlock(); return }
        finished = true
        lock.unlock()
        timer?.cancel()
        timer = nil
        stopTaps()
        lock.lock()
        defer { lock.unlock() }
        guard write else { return }
        sampleLocked(now: now)
        do { try log.write(to: url) } catch let f as RecordFailure { FileHandle.standardError.write(Data((f.message + "\n").utf8)) } catch {
            FileHandle.standardError.write(Data("could not write events.json: \(error.localizedDescription)\n".utf8))
        }
    }

    // MARK: sampling

    private func tick() {
        lock.lock()
        defer { lock.unlock() }
        guard !finished else { return }
        tickCount += 1
        let now = hostSeconds()
        sampleLocked(now: now)
        if tickCount % 6 == 0 { pollWindowsLocked(now: now) }
    }

    private func sampleLocked(now: Double) {
        guard let t = rel(now), let loc = CGEvent(source: nil)?.location else { return }
        if tickCount % 4 == 0 || tickCount == 0, let k = kinds { lastKind = k.current() }
        let (x, y) = local(loc)
        log.addCursor(t: t, x: x, y: y, kind: lastKind)
    }

    private func pollWindowsLocked(now: Double) {
        guard let t = rel(now),
              let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
                  as? [[String: Any]] else { return }
        var seen = 0
        for w in list {
            guard let num = (w[kCGWindowNumber as String] as? NSNumber)?.uint32Value,
                  let bd = w[kCGWindowBounds as String] as? NSDictionary,
                  let rect = CGRect(dictionaryRepresentation: bd) else { continue }
            if let only = config.windowID {
                if num != only { continue }
            } else {
                guard (w[kCGWindowLayer as String] as? NSNumber)?.intValue == 0, !config.excluded.contains(num),
                      rect.width >= 50, rect.height >= 50, rect.intersects(config.displayFrame) else { continue }
                seen += 1
                if seen > 40 { break }  // front-most windows only: bounded work and output
            }
            if lastFrames[num] == rect { continue }
            lastFrames[num] = rect
            let rel = CGRect(x: rect.origin.x - config.displayFrame.origin.x, y: rect.origin.y - config.displayFrame.origin.y,
                             width: rect.width, height: rect.height)
            log.addWindow(t: t, id: UInt64(num), frame: rel, app: w[kCGWindowOwnerName as String] as? String ?? "")
        }
    }

    // MARK: event taps (own thread + run loop; the helper's main thread is blocked reading stdin)

    private static let mouseTypes: [CGEventType] = [
        .leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp, .otherMouseDown, .otherMouseUp, .scrollWheel,
    ]
    private static let keyTypes: [CGEventType] = [.keyDown, .keyUp]

    private func makeTap(_ types: [CGEventType]) -> CFMachPort? {
        var mask: CGEventMask = 0
        for t in types { mask |= 1 << CGEventMask(t.rawValue) }
        let callback: CGEventTapCallBack = { _, type, event, refcon in
            guard let refcon = refcon else { return Unmanaged.passUnretained(event) }
            Unmanaged<InputRecorder>.fromOpaque(refcon).takeUnretainedValue().handle(type, event)
            return Unmanaged.passUnretained(event)
        }
        return CGEvent.tapCreate(tap: .cgSessionEventTap, place: .tailAppendEventTap, options: .listenOnly,
                                 eventsOfInterest: mask, callback: callback,
                                 userInfo: Unmanaged.passUnretained(self).toOpaque())
    }

    private func startTaps() {
        let mouse = makeTap(Self.mouseTypes)
        mouseTapAvailable = mouse != nil
        // The key tap is only attempted with the grant: never trigger the system prompt mid-recording.
        let key = CGPreflightListenEventAccess() ? makeTap(Self.keyTypes) : nil
        keysAvailable = key != nil
        taps = [mouse, key].compactMap { $0 }
        guard !taps.isEmpty else { return }
        let ready = DispatchSemaphore(value: 0)
        let thread = Thread { [weak self] in
            guard let self = self else { ready.signal(); return }
            let rl = CFRunLoopGetCurrent()
            for tap in self.taps {
                let src = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
                CFRunLoopAddSource(rl, src, .commonModes)
                CGEvent.tapEnable(tap: tap, enable: true)
            }
            self.tapRunLoop = rl
            ready.signal()
            CFRunLoopRun()
        }
        thread.name = "dev.ferrocast.capture.eventtap"
        thread.start()
        tapThread = thread
        _ = ready.wait(timeout: .now() + 2)
    }

    private func stopTaps() {
        for tap in taps { CGEvent.tapEnable(tap: tap, enable: false) }
        if let rl = tapRunLoop { CFRunLoopStop(rl) }
        taps = []
        tapRunLoop = nil
        tapThread = nil
    }

    private func handle(_ type: CGEventType, _ event: CGEvent) {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            for tap in taps { CGEvent.tapEnable(tap: tap, enable: true) }
            return
        }
        let now = hostSeconds()
        lock.lock()
        defer { lock.unlock() }
        guard !finished, let t = rel(now) else { return }
        let (x, y) = local(event.location)
        switch type {
        case .leftMouseDown, .rightMouseDown, .otherMouseDown, .leftMouseUp, .rightMouseUp, .otherMouseUp:
            let button = (type == .leftMouseDown || type == .leftMouseUp) ? "left"
                : (type == .rightMouseDown || type == .rightMouseUp) ? "right" : "other"
            let down = type == .leftMouseDown || type == .rightMouseDown || type == .otherMouseDown
            log.addClick(t: t, x: x, y: y, button: button, down: down)
        case .scrollWheel:
            log.addScroll(t: t, dx: Double(event.getIntegerValueField(.scrollWheelEventPointDeltaAxis2)),
                          dy: Double(event.getIntegerValueField(.scrollWheelEventPointDeltaAxis1)))
        case .keyDown, .keyUp:
            if event.getIntegerValueField(.keyboardEventAutorepeat) != 0 { return }
            let f = event.flags
            var mods: [String] = []
            if f.contains(.maskCommand) { mods.append("cmd") }
            if f.contains(.maskControl) { mods.append("ctrl") }
            if f.contains(.maskAlternate) { mods.append("alt") }
            if f.contains(.maskShift) { mods.append("shift") }
            // Decide before touching the key: a key without a shortcut modifier is never even named.
            guard mods.contains(where: { $0 != "shift" }) else { return }
            let name = keyName(Int(event.getIntegerValueField(.keyboardEventKeycode)))
            log.addKey(t: t, down: type == .keyDown, mods: mods, code: name.code, char: name.char)
        default:
            break
        }
    }
}
