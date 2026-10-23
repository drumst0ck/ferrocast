// Posts REAL mouse/keyboard events (CGEvent). Needs Accessibility for the hosting process.
// Coordinates are points, top-left origin of the main display.
import CoreGraphics
import Foundation

func fail(_ m: String) -> Never { FileHandle.standardError.write(Data((m + "\n").utf8)); exit(2) }
let a = Array(CommandLine.arguments.dropFirst())
func num(_ i: Int) -> Double { guard i < a.count, let v = Double(a[i]) else { fail("bad number at argument \(i + 1)") }; return v }
func pt(_ i: Int) -> CGPoint { CGPoint(x: num(i), y: num(i + 1)) }
func pause(_ ms: Int) { usleep(UInt32(ms * 1000)) }
let src = CGEventSource(stateID: .hidSystemState)

func mouse(_ t: CGEventType, _ p: CGPoint) {
    CGEvent(mouseEventSource: src, mouseType: t, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
}

func flags(_ s: String) -> CGEventFlags {
    var f = CGEventFlags()
    for m in s.lowercased().split(separator: "+") {
        switch m {
        case "cmd": f.insert(.maskCommand)
        case "shift": f.insert(.maskShift)
        case "alt": f.insert(.maskAlternate)
        case "ctrl": f.insert(.maskControl)
        default: fail("unknown modifier \(m)")
        }
    }
    return f
}

guard let cmd = a.first else { fail("usage: poster move X Y | click X Y | drag X1 Y1 X2 Y2 | key CODE [cmd+shift+alt+ctrl] | type TEXT") }
switch cmd {
case "move":
    mouse(.mouseMoved, pt(1))
case "click":
    let p = pt(1)
    mouse(.mouseMoved, p); pause(30)
    mouse(.leftMouseDown, p); pause(250); mouse(.leftMouseUp, p)
case "drag":
    let p1 = pt(1), p2 = pt(3), n = 12
    mouse(.mouseMoved, p1); pause(30)
    mouse(.leftMouseDown, p1); pause(50)
    for i in 1...n {
        let t = Double(i) / Double(n)
        mouse(.leftMouseDragged, CGPoint(x: p1.x + (p2.x - p1.x) * t, y: p1.y + (p2.y - p1.y) * t)); pause(16)
    }
    pause(50); mouse(.leftMouseUp, p2)
case "key":
    guard a.count >= 2, let code = UInt16(a[1]) else { fail("key needs a numeric key code") }
    let f = a.count > 2 ? flags(a[2]) : CGEventFlags()
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: src, virtualKey: CGKeyCode(code), keyDown: down)
        e?.flags = f
        e?.post(tap: .cghidEventTap)
        pause(30)
    }
case "type":
    guard a.count >= 2 else { fail("type needs text") }
    for u in Array(a[1...].joined(separator: " ").utf16) {
        var c = u
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: down)
            e?.keyboardSetUnicodeString(stringLength: 1, unicodeString: &c)
            e?.post(tap: .cghidEventTap)
            pause(8)
        }
    }
default:
    fail("unknown command \(cmd)")
}
