# Screen capture and Screen Studio-style recording for Ferrocast

Status: plan, not started. Written 2026-10-23 after a read-only review of the code and a spike
against the installed macOS SDK (see "Spike results"). Executor: Sonnet. Execute with the ODD
workflow: create `odd/tasks/screen-capture.md` from the Tasks section below before the first write.
This plan follows the house style of `odd/plans/apple-translation-plan.md` (Swift helper, gating,
specs, task table). Line numbers below are from the main checkout on 2026-10-23 and drift while other
agents merge: re-grep the quoted symbol before editing.

## Goal

Add to Ferrocast a capture suite that beats the native macOS one, and a screen recorder with an
editor that reproduces Screen Studio 4 (<https://screen.studio/>): automatic zooms that follow the
cursor, smoothed cursor, backgrounds, camera, microphone and system audio, transcript editing, and
export to MP4 and GIF. User request (Spanish, verbatim):

> "ahora vamos a empezar a hacer el planing para añadir a ferrocast una funcion nueva de captura de
> pantalla que mejore la nativa de mac, tambien grabara video y incluira funciones con zooms y
> funcionalidades de https://screen.studio/ haz el plan y lo ejecutara sonnet ten en cuenta que hay
> agentes haciendo mas cosas sobre este proyecto"

Scope is full parity with Screen Studio 4, delivered in six phases that each ship on their own:
P1 screenshot suite, P2 recording, P3 editor + auto-zoom + cursor + backgrounds + export, P4 camera
+ microphone + system audio, P5 transcript editing, P6 loupe / split screen / camera cutout / looks /
face tracking.

## Non-goals

- ffmpeg. It is optional in Ferrocast and not bundled (`crates/app/src/media_tools.rs:212`
  `preview_frame(ff: &str, …)` needs it). Nothing in this plan calls ffmpeg; rendering and export
  are native (ScreenCaptureKit, AVFoundation, Core Image/Metal, ImageIO).
- Supporting capture below macOS 14. The app keeps `LSMinimumSystemVersion` 13.0
  (`scripts/bundle.sh:30`); on 13 every command in this plan is hidden.
- Cloud features: sharing links, hosted uploads, accounts, sync. Everything stays on the Mac.
- Mobile/iPhone recording, live streaming, multi-track pro audio editing, custom keyframe curves UI.
- Replacing the existing Annotate, Frame, Trim Video or Crop Video tools. They are reused and
  untouched; the editor is a new view.
- Windows/Linux. Everything here is macOS-only; `ferrocast-core` code stays platform-neutral.
- Copying Screen Studio's assets, wallpapers, cursors or code. Ship our own gradients and cursors.
- Hardened-runtime/notarized release of the new permissions in this plan (see "Permissions"): the
  plan only documents what it would need.

## Screen Studio 4 feature inventory

The inventory is built from the user's list and public knowledge of the product; the product was not
scraped or run. Mark any row the user disputes in "Decisions to confirm".

| Feature | Phase | Our implementation |
| --- | --- | --- |
| Area / window / full-screen screenshot | P1 | Own selection overlay (`crates/macos/src/selector.rs`), helper `screenshot` via `SCScreenshotManager` |
| Annotate right after capture | P1 | Reuse `Act::AnnotateCanvas` and `ferrocast_core::annotate` |
| Background / padding / shadow on a screenshot | P1 | Reuse `frame::FrameOpts` and `app::frame_job` |
| Copy text from the screen (OCR) | P1 | Vision `VNRecognizeTextRequest` in `crates/macos/src/vision.rs` |
| Pin screenshot on top | P1 | NSPanel at floating level, `crates/macos/src/pin.rs` |
| Scrolling capture | P1 | Overlap stitching in core + synthetic scroll events |
| Record screen / area / window | P2 | Helper `record.start` (SCStream → AVAssetWriter), project folder |
| Cursor positions, clicks, keys, window frames | P2 | Helper sampler → `events.json` |
| Recording HUD, countdown, pause | P2 | NSPanel (glow.rs pattern) + tray state |
| Auto zoom from clicks | P3 | `ferrocast_core::capture::zoom` (pure) |
| Manual zoom segments, zoom level, follow cursor | P3 | EDL `zooms` with `mode: auto \| manual` |
| Cursor smoothing, size, click effect, hide when idle | P3 | `capture::cursor` (pure) + helper sprite rendering |
| Motion blur on zoom and cursor | P3 | `capture::cursor::blur_params` + Core Image sub-frame accumulation |
| Backgrounds (colour, gradient, image), padding, corners, shadow | P3 | EDL `style`, rendered by the helper; `FrameOpts` semantics shared |
| Trim, cut, speed ramps, speed up idle parts | P3 | EDL `trims`, `speeds`, `capture::timeline` |
| Export MP4 H.264/HEVC up to 4K60, GIF | P3 | Helper `render` (AVAssetWriter), GIF via ImageIO |
| Aspect ratios (16:9, 9:16, 1:1, 4:3…) | P3 | EDL `style.aspect`, letterboxed background |
| Camera overlay (shape, size, corner) | P4 | Helper camera capture to `camera.mov`, EDL `camera` |
| Microphone and system audio, separate tracks | P4 | `mic.m4a`, `system.m4a`, mix at export |
| Transcript, delete filler words / sentences, tighten pauses | P5 | On-device speech (SpeechAnalyzer / SFSpeechRecognizer), `capture::transcript` (pure) |
| Glass loupe | P6 | Core Image magnifier effect at EDL `loupes` |
| Split screen (screen + camera side by side) | P6 | EDL `camera.layout = "split"` |
| Camera cutout (person without background, beta) | P6 | Vision `VNGeneratePersonSegmentationRequest` |
| Camera colour looks (LUT) | P6 | `.cube` parser in core, `CIColorCubeWithColorSpace` |
| Face tracking (camera reframes on the face) | P6 | Vision face rectangles → `camera_track.json` |

## How it will work (user-facing)

### P1 Screenshot suite

1. The launcher lists **Capture Area**, **Capture Window**, **Capture Screen** and **Capture
   Scrolling** (subtitle `Capture`). Each can have a global shortcut in Settings → Capture (none by
   default; there is no default hotkey, unlike `DEFAULT_HOTKEYS`).
2. **Capture Area**: the launcher hides, every display dims with a crosshair, dragging draws a
   rectangle labelled `W × H`. <kbd>Space</kbd> switches to window mode (the window under the
   pointer is highlighted, click captures it), <kbd>Esc</kbd> cancels. **Capture Window** starts in
   window mode. **Capture Screen** captures the display under the pointer.
3. After the capture a **Capture** window (the launcher window, a new view) shows the picture and
   these actions: **Copy**, **Save**, **Annotate…** (opens the existing annotate editor on the
   file), **Background…** (existing frame tool: padding, corners, shadow, colour/gradient/image),
   **Copy Text** (OCR, the text goes to the clipboard), **Pin** (floating always-on-top window).
   <kbd>⏎</kbd> = Copy and close, <kbd>⌘S</kbd> = Save, <kbd>⌘T</kbd> = Copy Text, <kbd>⌘P</kbd> = Pin.
4. **Capture Scrolling**: pick an area, Ferrocast scrolls it by itself while capturing strips and
   stitches them. <kbd>Esc</kbd> or any click stops early; the result opens in the same Capture
   window.
5. Files are saved as PNG in the Capture folder (default `~/Pictures/Ferrocast Captures`) named
   `Capture 2026-10-23 at 14.05.22.png`.

### P2 Recording

1. **Record Screen / Record Area / Record Window** pick the target like P1, then show a 3-2-1
   countdown HUD, then record. A small HUD (stop, pause, elapsed time) floats above everything and
   is excluded from the recording; the menu-bar icon turns red with the elapsed time.
2. Stopping (HUD stop, tray menu, the command's own shortcut again) saves a project folder in
   `~/Movies/Ferrocast Recordings/<name>.ferrocast/` and opens it in the editor (P3) or, before P3
   ships, reveals it in Finder.
3. **Open Last Recording** opens the newest project. **Recordings** is a browser view: thumbnail
   grid, name, duration, size, date; actions Open, Reveal, Rename, Move to Trash.

### P3 Editor

The editor is a launcher view with a preview on top, a timeline underneath and a side panel with
tabs **Zoom**, **Cursor**, **Style**, **Audio**, **Export**. Auto-zoom segments are created when the
project opens the first time and can be dragged, resized, deleted or switched to manual (fixed
region chosen by dragging on the preview). The cursor is redrawn smooth and bigger. **Export**
offers MP4 (H.264 or HEVC, 1080p/1440p/4K, 30/60 fps) and GIF (max 960 px, 15 fps), with progress
and Cancel; the file goes next to the project and is revealed.

### P4–P6

P4 adds camera, microphone and system audio toggles to the recording start sheet and a **Camera**
and **Audio** panel to the editor. P5 adds a **Transcript** tab: words listed with timestamps,
select and delete words or sentences (they are cut from the video), buttons **Remove filler words**
and **Tighten pauses**. P6 adds loupe segments on the timeline, camera layouts (split), cutout,
**Looks** and face tracking toggles.

### macOS 13

Commands are hidden. A stored shortcut for a capture command still fires and shows once per session
the toast `Screen capture needs macOS 14. This Mac runs macOS 13.` (S3). The Capture tab shows the
same sentence in place of its controls.

## Architecture

### Pieces and ownership

| Piece | Where | Owns |
| --- | --- | --- |
| Swift helper `ferrocast-capture` | `crates/macos/swift/capture/*.swift` | ScreenCaptureKit, AVFoundation, Core Image/Metal, ImageIO, Speech, Vision-on-video |
| Rust helper client | `crates/macos/src/capture.rs` (new) | spawn, NDJSON framing, timeouts, kill, version gate |
| AppKit panels | `crates/macos/src/{selector,hud,pin}.rs` (new) | selection overlay, recording HUD, pinned window |
| Pure model and maths | `crates/core/src/capture/*.rs` (new) | project format, EDL validation, zoom/cursor/timeline maths, stitching, OCR layout, protocol types |
| UI | `crates/app/src/{app,draw,items,services}.rs` | views, commands, settings tab, bus messages |

`ferrocast-core` has no macOS code; every number that decides how the video looks is computed there
and sent to Swift as data. Swift never decides *where* the camera goes, only *draws* it. That keeps
the maths unit-testable (`cargo test`, no GPU, no permissions) and the renderer replaceable.

### Swift helper `ferrocast-capture`

- Sources: `crates/macos/swift/capture/{main,Protocol,Shareable,Screenshot,Recorder,Events,Render,Export,Camera,Audio,Transcribe,Analyze}.swift`
  (multi-file; `main.swift` is the entry, so compile **without** `-parse-as-library`; the spike
  showed top-level code needs `main.swift`). Files appear phase by phase; P1 ships only
  `main, Protocol, Shareable, Screenshot`.
- Built by `scripts/bundle.sh` into `Contents/MacOS/ferrocast-capture`, signed with the same
  identity and flags as the app (identifier `dev.ferrocast.launcher.capture`), before the app bundle
  is signed. Build line:
  `xcrun swiftc -O -target arm64-apple-macos13.0 -Xlinker -weak_framework -Xlinker ScreenCaptureKit
  -Xlinker -weak_framework -Xlinker Speech … crates/macos/swift/capture/*.swift -o …`.
  Weak-linking is verified by `otool -L` in the spike (ScreenCaptureKit shows `weak`). Every
  framework use sits behind `if #available(macOS 14, *)` and the binary answers
  `{"error":"unsupported-os"}` to every command except `hello` on macOS 13.
- Arm64 only, like the rest of the app. If the build gets a universal target, add `-target
  x86_64-apple-macos13.0` and `lipo`; not in scope.
- **Long-lived**: one process per recording session or editor session, started lazily by
  `capture.rs`, killed on app quit and when its stdin closes (the helper exits on EOF). One-shot
  commands (`screenshot`, `shareable`) reuse the same process when alive, otherwise spawn a
  short-lived one.

### Protocol (newline-delimited JSON, UTF-8, one object per line)

Requests carry an `id` (u64, chosen by Rust). Responses repeat it. Events (`"event"`) carry the `id`
of the request that started them. Text and paths travel in JSON on stdin, never in argv. stdout is
protocol only; the helper logs to stderr, which `capture.rs` appends to `ferrocast.log`
(`instance::log`, `crates/app/src/instance.rs:26`). Errors use `{"id":N,"error":"<code>","message":"…"}`
with codes `unsupported-os`, `permission-screen`, `permission-camera`, `permission-mic`,
`not-found`, `busy`, `cancelled`, `disk-full`, `invalid`, `internal`. Exit codes: 0 clean, 1 fatal,
2 unsupported OS (only if it cannot even answer).

```json
{"id":1,"cmd":"hello"}
→ {"id":1,"ok":true,"protocol":1,"os":"27.0.0","capabilities":["screenshot","record","render","ocr"],
   "recording_output":true,"speech_analyzer":true,"metal":true}

{"id":2,"cmd":"shareable"}
→ {"id":2,"ok":true,"displays":[{"id":1,"frame":[0,0,1728,1117],"scale":2.0,"main":true,"name":"Built-in"}],
   "windows":[{"id":4231,"app":"Safari","bundle":"com.apple.Safari","title":"Docs","frame":[120,80,1200,800],"layer":0,"on_screen":true}]}

{"id":3,"cmd":"screenshot","target":{"kind":"rect","display":1,"rect":[100,100,640,400]},
 "out":"/Users/me/Pictures/Ferrocast Captures/Capture.png","cursor":false,"window_shadow":false,"scale":"native"}
→ {"id":3,"ok":true,"path":"…/Capture.png","size":[1280,800],"scale":2.0}
   target kinds: {"kind":"display","display":1} | {"kind":"window","window":4231} | {"kind":"rect",…}
   rect is in display-local points, top-left origin (the same space as `FrameOpts` pixels / 2).

{"id":4,"cmd":"ocr","path":"/tmp/x.png"}          // P1 uses Rust Vision instead; see "OCR"; command kept for P5 frames
{"id":5,"cmd":"record.start","project":"/Users/me/Movies/Ferrocast Recordings/Demo.ferrocast",
 "target":{"kind":"display","display":1},"fps":60,"cursor_hidden_in_video":true,
 "camera":null,"mic":null,"system_audio":false,"excludes":{"window_ids":[9001]}}
→ {"id":5,"ok":true,"started_at_host":123456.789}          // then events:
   {"event":"state","id":5,"state":"recording","t":0.0}
   {"event":"tick","id":5,"t":12.5,"bytes":48211000,"dropped":0}
   {"event":"warn","id":5,"code":"low-disk","free_bytes":900000000}
{"id":6,"cmd":"record.pause"}  {"id":7,"cmd":"record.resume"}
{"id":8,"cmd":"record.stop"}
→ {"id":8,"ok":true,"project":"…","duration":73.2,"screen":"screen.mov","size":[3456,2234]}

{"id":9,"cmd":"render.frame","project":"…","t":12.34,"width":960,"out":"…/.cache/f-1234.png"}
→ {"id":9,"ok":true,"path":"…/.cache/f-1234.png","size":[960,540],"ms":22}

{"id":10,"cmd":"render","project":"…","format":"mp4","codec":"hevc","size":[3840,2160],"fps":60,
 "quality":"high","range":[0,73.2],"out":"…/Demo.mp4"}
→ events {"event":"progress","id":10,"fraction":0.42,"eta":18.0} … {"id":10,"ok":true,"path":"…","bytes":88000000}
{"id":11,"cmd":"cancel","target":10}

{"id":12,"cmd":"devices"}   // P4
→ {"id":12,"ok":true,"cameras":[{"id":"…","name":"FaceTime HD"}],"mics":[{"id":"…","name":"MacBook Pro Microphone"}]}
{"id":13,"cmd":"transcribe","project":"…","locale":"en-US"}   // P5, events "progress", result words[]
→ {"id":13,"ok":true,"words":[{"w":"hello","s":0.52,"e":0.81,"c":0.93}],"engine":"speech-analyzer"}
{"id":14,"cmd":"analyze.face","project":"…"}                    // P6 → writes camera_track.json
```

Pure encode/decode of these structs lives in `ferrocast_core::capture::protocol` (serde, tagged by
`cmd`) and is tested without macOS (round trips, unknown field tolerance, error-code mapping).
`render.frame` frames go through files, not stdout, so a 4K PNG never blocks the pipe; the helper
deletes `.cache/` files older than 60 s and the whole `.cache` on project close.

### Rust client `crates/macos/src/capture.rs`

`Helper::spawn() -> Result<Helper>`, `request(cmd) -> Receiver<Reply>` (one reader thread splitting
lines into replies and events, one writer), `kill()`. Timeouts: `hello` 5 s, `shareable` 10 s,
`screenshot` 20 s, `render.frame` 10 s, `render` none (progress heartbeats; a 30 s silence kills it),
`record.*` 10 s. Reuse the kill-on-timeout pattern of `system::osascript_timeout`
(`crates/macos/src/system.rs:20`). `helper_path()` resolves next to `current_exe` through
`instance::resolve_exe` exactly as `apple-translation-plan.md` does for `ferrocast-translate`;
`available()` = `os_at_least(14)` and helper executable. `os_at_least(major)` is shared with the
translation plan: whichever lands first adds `ferrocast_macos::system::os_at_least`, the other
reuses it (see "Concurrency").

### OCR

`crates/macos/src/vision.rs` already wraps Vision for faces and text *rectangles*
(`vision.rs:6`). `crates/macos/Cargo.toml` enables `VNDetectTextRectanglesRequest` but not
`VNRecognizeTextRequest`; that feature exists in `objc2-vision` 0.3.2 (checked in the registry
`Cargo.toml`). T-OCR adds the features `VNRecognizeTextRequest` and the observation/candidate ones
the compiler asks for (`VNRecognizedTextObservation`, `VNRecognizedText`), plus
`fn recognize_text(path) -> Result<Vec<Line>>` with `recognitionLevel = accurate`,
`usesLanguageCorrection = true`, `automaticallyDetectsLanguage = true` (macOS 13+ API; guarded). Pure
reading-order and paragraph joining (`capture/ocr.rs`: sort lines by band, join hyphenated line
ends, collapse double spaces) is in core and tested with synthetic boxes. As in `vision.rs:57-60`
tests, skip (do not fail) when Vision reports "inference context" (virtual CI machines).

### Project format (`~/Movies/Ferrocast Recordings/<name>.ferrocast/`)

```
Demo.ferrocast/
  screen.mov        raw screen, HEVC or H.264 at capture size, no cursor
  camera.mov        optional (P4)
  mic.m4a           optional (P4)
  system.m4a        optional (P4)
  events.json       what the user did, 60 Hz
  project.json      the edit decision list (EDL)
  camera_track.json optional (P6)
  thumb.png         first frame, 480 px, for the Recordings browser
  .cache/           preview frames, deletable
```

`events.json` (all times in seconds from the first screen sample, `t = 0`):

```json
{"version":1,"screen":{"size":[3456,2234],"scale":2.0,"display_frame":[0,0,1728,1117]},
 "cursor":[[0.000,812.5,400.2,"arrow"],[0.016,813.1,401.0,"arrow"]],
 "clicks":[{"t":1.42,"x":812.5,"y":400.2,"button":"left","down":true},{"t":1.51,"up":true,"x":812.5,"y":400.2,"button":"left"}],
 "keys":[{"t":2.10,"down":true,"mods":["cmd"],"code":"KeyC","char":"c"}],
 "scroll":[{"t":4.2,"dx":0,"dy":-12.0}],
 "windows":[{"t":0.0,"id":4231,"frame":[120,80,1200,800],"app":"Safari"}]}
```

Coordinates are points relative to the recorded display, top-left origin. Key *contents* are stored
only for shortcuts (any modifier except shift) and never for plain typing: plain keys keep `t` and
`down` only, so a recording never contains passwords. Key capture needs Input Monitoring; without
it `keys` is empty and the recorder says so in the start sheet (S9).

`project.json` (EDL; `version` 1, unknown fields kept on save so newer files survive older builds):

```json
{"version":1,"name":"Demo","created":"2026-10-23T14:05:22Z","duration":73.2,
 "sources":{"screen":{"file":"screen.mov","offset":0.0},"camera":null,"mic":null,"system":null},
 "trims":[{"start":0.0,"end":73.2}],
 "speeds":[{"start":30.0,"end":38.0,"factor":4.0}],
 "zooms":[{"id":"z1","start":1.0,"end":4.6,"mode":"auto","scale":2.0,"focus":null,"ease_in":0.6,"ease_out":0.7}],
 "cursor":{"style":"arrow","size":1.4,"smoothing":0.5,"click_effect":"ring","hide_idle_after":3.0,"motion_blur":0.5},
 "style":{"aspect":"16:9","padding":0.06,"radius":18,"shadow":{"blur":40,"opacity":0.5,"dy":16},
          "background":{"kind":"gradient","stops":["#1f1c2c","#928dab"],"angle":135}},
 "camera":null,"audio":{"mic_gain":1.0,"system_gain":0.6,"mute":false},
 "loupes":[],"transcript":null,"export":{"format":"mp4","codec":"h264","size":[1920,1080],"fps":60}}
```

`style.background.kind`: `color` | `gradient` | `image` (path inside the project or absolute) |
`wallpaper` (one of ~8 bundled gradients generated in code, no image assets). `focus` is `null` for
auto (the maths picks the centre from cursor events) or `[x,y]` normalised for manual.

### Core modules `crates/core/src/capture/` (register `pub mod capture;` in `crates/core/src/lib.rs` next to `pub mod calc;`, alphabetical)

| Module | Pure content |
| --- | --- |
| `mod.rs` | re-exports, `MIN_SEGMENT` (reuse `videoplan::MIN_CLIP`, `crates/core/src/videoplan.rs:9`) |
| `protocol.rs` | request/reply/event structs and error codes above |
| `project.rs` | `Project`, `Edl` types, load/save, `validate()` (below), migration by `version` |
| `events.rs` | `Events` types, decode, resample to a fixed rate, clamp to display, `idle_spans()` |
| `curves.rs` | critically damped spring (analytic, no integration drift), cubic ease, `lerp`, `smoothstep`; deterministic |
| `zoom.rs` | `auto_zooms(&Events, &ZoomOpts) -> Vec<Zoom>`, `camera_at(&[Zoom], &Events, t) -> Viewport` |
| `cursor.rs` | `smooth(&[Sample], strength) -> Vec<Sample>`, `blur_params(velocity, zoom_speed) -> Blur`, hide-when-idle |
| `timeline.rs` | source↔output time mapping through trims and speeds, `output_duration`, `snap`, `Edit` + undo stack |
| `region.rs` | selection rect normalising, window snapping, display-space flips (P1) |
| `stitch.rs` | scrolling-capture overlap detection and strip merge on RGBA buffers (P1) |
| `ocr.rs` | line ordering and joining (P1) |
| `transcript.rs` | filler-word detection, pause tightening, word range → cut ranges (P5) |
| `lut.rs` | `.cube` parser, 3D table validation (P6) |

Algorithms to implement and test (all numbers are defaults in `ZoomOpts`/`CursorOpts`, each tunable
in Settings → Capture → Auto zoom):

- **Auto zoom** (`zoom.rs`): take click-down events; merge clicks into one *cluster* while the gap
  to the previous click is ≤ 1.5 s and the distance is ≤ 25 % of the display width; each cluster
  becomes a zoom starting `lead = 0.4 s` before its first click and ending `idle = 1.2 s` after its
  last click or last cursor movement larger than 8 px/frame, whichever is later; drop clusters
  shorter than 0.5 s; two zooms closer than 0.8 s merge. `scale` default 2.0, clamped so the
  viewport stays inside the display. Typing bursts (≥ 5 keys within 2 s, from `keys` timestamps)
  also create a zoom centred on the focused text caret when `windows`/`cursor` is the only anchor:
  v1 centres on the last cursor position (no accessibility caret lookup).
- **Viewport follow**: while a zoom is active the viewport centre follows the smoothed cursor only
  when the cursor leaves the inner 60 % box of the viewport (dead zone), moved by the spring; it
  never leaves the display. Zoom-in and zoom-out use the spring with `response = ease_in/ease_out`
  and no overshoot (critically damped).
- **Cursor smoothing** (`cursor.rs`): resample to the output frame rate, then a spring-follower with
  `strength` 0..1 mapped to a time constant of 0..120 ms; clicks keep their exact timestamps; the
  smoothed cursor must never lag a click by more than 1 frame (test).
- **Motion blur**: `blur_params` returns `{samples, shutter}` from the screen-space speed of the
  camera and the cursor (0 below 200 px/s, up to 8 samples, shutter angle 180°). The helper renders
  `samples` sub-frames inside one output frame and averages them.
- **EDL validation** (`Project::validate`): `trims` sorted, non-overlapping, inside `0..duration`,
  each ≥ `MIN_SEGMENT`; `speeds` inside the kept ranges, factor in `0.25..=16`; `zooms` non-overlapping
  (auto may be trimmed by manual), `scale` in `1.0..=6.0`, `focus` in `0..=1`; background colours
  `#rrggbb`; `export.size` even numbers ≤ 3840×2160, `fps` ∈ {24,30,60}; unknown `version` > 1 is
  rejected with the message `This recording was made with a newer Ferrocast.`

### Rendering (Swift, `Render.swift`)

Per output frame at output time `to`: map to source time `ts` (`timeline` data is sent inside the
request; Swift only gets the *already evaluated* camera track: the Rust side samples
`camera_at` and `cursor::smooth` at the output fps and passes arrays `[{to, ts, vx, vy, vs}]`
in the request for `render`, or computes a single sample for `render.frame`). Pipeline in Core Image
with a Metal-backed `CIContext`: source frame → crop/scale by viewport → rounded-rect mask → drop
shadow → composite over the background (cached once per render) → cursor sprite (own vector arrow,
I-beam, pointer hand and crosshair drawn in code; `kind` from `events.json`) → click ring →
camera bubble (P4) → loupe (P6) → motion-blur accumulation. If `MTLCreateSystemDefaultDevice()` is
nil the context falls back to the CPU renderer and `hello.metal=false` (slow, warned in the UI).
Export writes through `AVAssetWriter` with `AVVideoCodecType.h264` or `.hevc`, `fps`, bitrate by
`quality`, audio mixed from `mic.m4a`/`system.m4a` with the speed map. GIF: frames at 15 fps, max
960 px wide, `CGImageDestination` with `kCGImagePropertyGIFLoopCount = 0`. 4K60 H.264 is capped at
level 5.2 settings; HEVC is offered first when the machine has a hardware encoder.

### Recorder (Swift, `Recorder.swift`, `Events.swift`)

- Capture with `SCStream` + `SCStreamConfiguration` (`showsCursor = false`, `queueDepth = 6`,
  `minimumFrameInterval = 1/fps`, `pixelFormat = 420v` for the encoder, `colorSpaceName = displayP3`
  on wide-gamut displays) and write samples with `AVAssetWriter`/`AVAssetWriterInput`. Not
  `SCRecordingOutput`: it is macOS 15 only (spike), cannot write the cursor-free separate tracks we
  need, and the plan needs macOS 14.
- Frame pacing: `SCStreamFrameInfo.status` `.idle` frames are skipped; the writer uses source PTS, so
  a still screen costs nothing and duration is correct. Dropped frames counted in `tick`.
- One clock: every file's times derive from `CMClockGetHostTimeClock()`; `project.json.sources.*.offset`
  stores each source's first PTS minus the screen's first PTS.
- `events.json` is sampled by a `DispatchSourceTimer` at 60 Hz reading `NSEvent.mouseLocation`
  (no permission needed) plus global `NSEvent` monitors for mouse buttons and scroll; keys use a
  `CGEventTap` listen-only only when `CGPreflightListenEventAccess()` (already used at
  `crates/macos/src/windows.rs:257`) is true. Cursor kind comes from `NSCursor.currentSystem`
  compared with the system cursors by image hash; unknown shapes are stored as `"arrow"`. **Unverified
  on macOS 14–27: confirm in T14.**
- Exclusion of Ferrocast's own HUD: `excludes.window_ids` (the HUD panel's `windowNumber`) goes into
  `SCContentFilter(display:excludingWindows:)`.
- App Nap: the helper calls `ProcessInfo.beginActivity(options: [.userInitiated, .latencyCritical])`
  while recording and holds an IOPM assertion `PreventUserIdleDisplaySleep`.

### UI in the app

- **Views**: add to `enum View` (`crates/app/src/app.rs:315` region, beside `Annotate`,
  `VideoSpeed`, `VideoRedact`, `VideoCrop`) the variants `CaptureResult { file, size }`,
  `Recordings`, `Editor { project }`. Add `Act::OpenEditor(PathBuf)`, `Act::PinImage(PathBuf)`,
  `Act::CopyText(PathBuf)`, `Act::RunCapture(CaptureCmd)` next to `Act::AnnotateCanvas`
  (`app.rs:4774` handler, `app.rs:3287` and `app.rs:3425` action lists).
- **Opening existing editors**: Annotate from the Capture window is `self.perform(Act::AnnotateCanvas(file), false)`
  (same call as `app.rs:1725` and `app.rs:3796`); Background is the existing frame tool whose job is
  built by `app::frame_job` (`app.rs:125`) from `FrameOpts` (`crates/core/src/frame.rs:14`,
  `Default` pad 64, radius 0, shadow 0); no new image code in P1.
- **Editor preview**: egui textures from `render.frame` PNGs decoded with `services::decode_png`
  (the same decode `draw.rs:268/346` use for `preview_frame` output). Scrubbing asks for frames at
  960 px, coalescing requests (only the latest `t` is kept); playback requests ahead at 30 fps with
  a 4-frame prefetch and drops to 15 fps when the average reply exceeds 45 ms. Final export uses
  `render` streaming progress through a new `Msg::CaptureProgress(f32)` /
  `Msg::CaptureDone(Result<PathBuf,String>)` on the existing `Bus` (`crates/app/src/services.rs:28`
  `enum Msg`).
- **Recording HUD**: `crates/macos/src/hud.rs`, an `NSPanel` subclass defined like `GlowPanel`
  (`crates/macos/src/glow.rs:92-105`: borderless + `NonactivatingPanel`, `canBecomeKeyWindow = false`),
  main-thread only (`MainThreadMarker`), pure layout functions unit-tested like `layout_for`
  (`glow.rs:67`). Unlike the glow it accepts mouse clicks (Stop/Pause buttons). Level
  `NSPopUpMenuWindowLevel`, `sharingType = .none` (not an excuse to skip `excludes`).
- **Tray**: `services::make_tray` (`crates/app/src/services.rs:652`) gets `set_recording(Some(secs)|None)`
  swapping the icon to a red-dot variant and setting the title to `●  00:12`; the tray menu gains
  **Stop Recording** while active. Keep the `Option<tray_icon::TrayIcon>` field (`app.rs:777`).
- **Selection overlay** `crates/macos/src/selector.rs`: one borderless `NSPanel` per `NSScreen` at
  `NSScreenSaverWindowLevel`, key-accepting for Esc/Space, crosshair cursor, dimming layer with a
  hole for the rectangle; window highlight from `shareable.windows`. Returns a `Selection` through a
  callback to the app's `Bus`. Geometry in `capture::region` (pure).
- **Pin**: `crates/macos/src/pin.rs`, panel at `NSFloatingWindowLevel`, `collectionBehavior`
  `CanJoinAllSpaces | FullScreenAuxiliary`, draggable by background, scroll to zoom 25–400 %,
  <kbd>Esc</kbd> or double-click closes, right-click menu (Copy, Opacity, Close). Same pattern as
  `glow.rs` but interactive.
- **Settings → Capture tab**: `SETTINGS_TABS` (`crates/app/src/draw.rs:6547`) is a `[(&str,&str); 7]`
  with `SETTINGS_GROUP_STARTS = [3, 5]` today and `SETTINGS_NUM_KEYS: [Key; 7]` (`draw.rs:6557-6558`).
  Add `("Capture", "sf:camera.viewfinder")` after Translate (index 5), shifting Scripts/Advanced to
  6/7; update the three constants, `SETTINGS_GROUP_STARTS` to `[3, 5, 6]` (Capture is its own group,
  **decision 6**) and the hard-coded tab indices in
  `crates/core/src/settings_index.rs` (`e(5…)`→`e(6…)`, `e(6…)`→`e(7…)`, header comment lines 1-5)
  and in the test `sd3_every_drawn_row_is_indexed_and_every_entry_is_drawn`
  (`crates/app/src/tests.rs:7145`, loops `[0usize, 1, 2, 5, 6]`). This renumbering touches code other
  agents edit; it is isolated in its own task (T9) to rebase cleanly.
- **Commands**: `Builtin` (`crates/app/src/items.rs:21`) gets `CaptureArea, CaptureWindow,
  CaptureScreen, CaptureScrolling, RecordScreen, RecordArea, RecordWindow, OpenLastRecording,
  Recordings`; update `ALL` (`items.rs:55`), `title()` (`items.rs:91`), the category match
  (`items.rs:124-136`, new category `"Capture"`), the icon match (`items.rs:146-162`, e.g.
  `sf:camera.viewfinder`, `sf:record.circle`), `keywords()` (`items.rs:177-191`), and the executor in
  `app.rs` beside `Builtin::ConvertFiles` (`app.rs:4111`). `Builtin::id()` (`items.rs:196`) yields
  `builtin:capture-area` etc. Hotkeys use the existing per-command mechanism: `Settings::hotkey(id)`
  (`models.rs:347`), registered by `Hotkeys::apply` (`services.rs:560`); no entry in
  `DEFAULT_HOTKEYS` (`models.rs:189`). Hidden on macOS 13 by filtering where the builtin items are
  built (`app.rs:1335-1342` region), using `capture::available()`.
- **Settings model**: new `#[serde(default)] pub capture: CaptureSettings` on `Settings`
  (`models.rs:205-207`, struct is `#[serde(default)]`; add the field to `Default` at `models.rs:292-337`)
  with `folder: String` (empty = default), `screenshot_format: png|jpg`, `after: show|copy|save`,
  `include_cursor: bool`, `recordings_folder`, `fps: 30|60`, `default_background`, `cursor_size`,
  `auto_zoom: bool`, `zoom_scale`, `camera_id`, `mic_id`, `system_audio`, `countdown: 0|3|5`.
  Old files load unchanged (S2).
- **Settings index**: rows added in `settings_index.rs` (`ENTRIES`, style
  `e(tab, "Title", &["kw",…])`) for every drawn row; the sd3 test enforces parity.
- **Permissions**: see below.

### Permissions, Info.plist, entitlements

- Screen Recording: `CGPreflightScreenCaptureAccess()` (spike: returns `false` for an unauthorised
  CLI) before every capture; `CGRequestScreenCaptureAccess()` once, then open
  `x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture`. The helper maps
  SCStream error `-3801` (spike: `El usuario ha rechazado las preferencias de TCC…`) to
  `permission-screen`. Toast (S4): `Screen Recording is off for Ferrocast. Turn it on in System
  Settings → Privacy & Security → Screen & System Audio Recording, then try again.`
- Camera and microphone (P4): `AVCaptureDevice.requestAccess`; strings in the toast per S12.
- Accessibility (already used by window management, `windows.rs:250`): needed only to post scroll
  events for scrolling capture; Input Monitoring only for key events in `events.json`.
- TCC attribution: the helper is spawned by Ferrocast, so macOS normally charges the *responsible
  process* (the app) and the prompt/Settings row says "Ferrocast". Verified only for Automation in
  this repo; **T3's smoke script must check it from the signed bundle** and, if the helper gets its
  own row, add the helper identifier to the toast.
- `scripts/bundle.sh` Info.plist additions (heredoc at `bundle.sh:20-35`):
  `NSCameraUsageDescription` "Ferrocast records your camera only when you turn it on in a
  recording.", `NSMicrophoneUsageDescription` "Ferrocast records your microphone only when you turn
  it on in a recording.", `NSSpeechRecognitionUsageDescription` "Ferrocast transcribes your
  recording on this Mac to let you edit it by text.", `NSScreenCaptureUsageDescription` (ignored on
  14+, harmless). The existing `NSAppleEventsUsageDescription` stays.
- `resources/Ferrocast.entitlements` today holds only `com.apple.security.automation.apple-events`.
  The app is ad-hoc signed and not sandboxed, so **no new entitlement is needed to run**. For a
  Developer ID hardened-runtime release (`--options runtime`, `bundle.sh:40`) add
  `com.apple.security.device.camera` and `com.apple.security.device.audio-input` to this file; screen
  recording needs none. Adding them is safe for ad-hoc builds. The helper is signed with the same
  `flags` so it inherits the same set.

### Build, signing, CI

- `scripts/bundle.sh`: after `cargo build`, a function `build_swift_helper <name> <src_dir> <weak
  frameworks…>` (shared with `ferrocast-translate`; if the translation plan has not landed, T3 adds
  the function and that plan reuses it) compiles `ferrocast-capture` when `xcrun --show-sdk-version`
  ≥ 14, signs it with `flags` plus `-i dev.ferrocast.launcher.capture`, before the app is signed
  (`bundle.sh:47`). Missing/old SDK: print `NOTE: ScreenCaptureKit SDK not available; capture is disabled`
  and continue (Homebrew builds on the user's machine keep working).
- `scripts/capture-smoke.sh` (new, no `swift test`): builds the helper into a temp dir, runs `hello`,
  `shareable` (accepts `permission-screen` as pass on a runner without permission), a 2-second
  `record.start/stop` only when `CGPreflightScreenCaptureAccess` is true, renders one frame from a
  fixture project in `crates/core/fixtures/capture/demo.ferrocast/` (a 1-second 320×200 H.264 .mov
  made once with the helper and committed, < 100 KB), and asserts the PNG size. Exits 0 with
  `SKIP` lines when no permission. Called from CI after the bundle step.
- `.github/workflows/ci.yml:8` is `runs-on: macos-14` on `origin/master` (the user believes the
  runners are macOS 26; `apple-translation-plan.md` proposes moving to `macos-26`). The helper only
  needs SDK 14 to build, so P1–P3 keep `macos-14`, **but** SpeechAnalyzer (P5) and any `#available(macOS 26)`
  code compile only with SDK 26: guard those with `#if compiler(>=6.2)` plus `canImport` or skip
  them when `xcrun --show-sdk-version` < 26, and decide the runner change together with the
  translation plan (decision 3). Add a CI step `test -x dist/Ferrocast.app/Contents/MacOS/ferrocast-capture`.
- `Formula/ferrocast.rb`: no dependency (swiftc ships with CLT); caveat: capture needs macOS 14.
- `scripts/notarize.sh`, `release.yml`: nothing changes if the helper is signed with the shared
  `flags`; verify with `codesign --verify --strict --deep` (as in the translation plan). Own commit,
  review risk item 5 (delivery).

### Docs

- `web/src/content/docs/docs/launcher/screen-capture.md` and
  `web/src/content/docs/es/docs/launcher/screen-capture.md` (format of `translate.md`: frontmatter
  `title`, `description` of **120–155 characters** (count with `wc -m`), `sidebar.order`, then
  prose with <kbd> tags). One page covers all phases; each phase's docs task appends its section.
  CI runs `npm run gen:docs` (`web/package.json`, `tsx scripts/gen-docs.ts`) and diffs: run it in
  `web/` and commit generated output with the page.
- `web/src/content/docs/docs/reference/settings.md` EN/ES: Capture tab rows.
- `CHANGELOG.md` `## [Unreleased]` → `### Added`, one bullet per shipped phase, with an optional
  `**Title**:` prefix and `(docs: /docs/launcher/screen-capture)`.
- `docs/` (repo): add the permissions note if a permissions page exists.

## Specs

- S1 On macOS ≥ 14 with the helper present, the launcher lists `Capture Area`, `Capture Window`,
  `Capture Screen`, `Capture Scrolling` (P1), `Record Screen`, `Record Area`, `Record Window`,
  `Open Last Recording`, `Recordings` (P2), each with subtitle `Capture`. Commands appear only in
  the phase that implements them.
- S2 Existing `settings.json` files load unchanged; a missing `capture` object gives the defaults
  (folder `~/Pictures/Ferrocast Captures`, PNG, 60 fps, 3 s countdown, auto zoom on, scale 2.0).
- S3 On macOS 13 or without the helper the commands are hidden, and a stored shortcut shows once per
  session: `Screen capture needs macOS 14. This Mac runs macOS 13.` (helper missing:
  `Screen capture is not available in this build.`)
- S4 Without Screen Recording permission the toast is exactly: `Screen Recording is off for
  Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording,
  then try again.` and the Settings pane opens; no half-written file is left.
- S5 Capture Area draws a dimmed overlay on every display, a crosshair, a `W × H` label in pixels,
  <kbd>Space</kbd> toggles window mode, <kbd>Esc</kbd> cancels with no file and no toast.
- S6 A capture saves `Capture YYYY-MM-DD at HH.MM.SS.png` (collision → ` 2`), at the display's native
  pixel size (Retina 2×), in the Capture folder, then opens the Capture window (or only copies, or
  only saves, per `after`).
- S7 The Capture window offers Copy, Save, Annotate…, Background…, Copy Text, Pin with the shortcuts in
  "How it will work"; Annotate and Background use the existing editors on the saved file.
- S8 Copy Text puts recognised text on the clipboard in reading order and toasts `Copied N characters`
  or `No text found`; nothing leaves the Mac.
- S9 Pin opens a borderless always-on-top window of the image at its captured size (points),
  draggable, zoomable 25–400 %, closed by <kbd>Esc</kbd>; several pins can coexist.
- S10 Capture Scrolling stitches vertically scrolling content with no visible seam and no duplicated
  rows for content that scrolls ≥ 1 row per step; it stops at the end of the scroll (two identical
  strips) or after 60 steps, with the toast `Scrolling capture stopped at 60 screens` for the cap.
- S11 Recording writes a project folder with `screen.mov`, `events.json`, `project.json`,
  `thumb.png`; duration of `screen.mov` is within 0.1 s of the wall-clock time between countdown end
  and stop, even when the screen is static.
- S12 Permission toasts: camera `Camera is off for Ferrocast. Turn it on in System Settings →
  Privacy & Security → Camera.`; microphone the same with `Microphone`; Input Monitoring missing →
  the start sheet says `Keyboard shortcuts will not be shown: Input Monitoring is off.` and records.
- S13 The HUD shows elapsed `mm:ss`, Pause, Stop; it is not in the recording; the tray shows
  `●  mm:ss`; the free-disk warning appears at < 2 GB with `Low disk space: N GB left` and recording
  stops cleanly at < 200 MB keeping the file.
- S14 Auto zoom is deterministic: the same `events.json` and options produce byte-identical `zooms`
  (pure function, golden test), zoom scale never exceeds the setting, the viewport never leaves the
  display, and a zoom starts 0.4 s before its first click.
- S15 The editor opens a project with auto-zoom applied, plays it smoothly (≥ 24 fps preview at
  960 px on Apple silicon), supports undo/redo (⌘Z/⇧⌘Z) for every EDL change, and autosaves
  `project.json` atomically (write temp + rename) 1 s after the last change.
- S16 Export MP4 (H.264/HEVC, up to 3840×2160 @ 60) and GIF (≤ 960 px, 15 fps) with progress and a
  Cancel that leaves no partial output; the output duration equals `timeline::output_duration`
  within one frame; audio stays in sync (±40 ms) with speed changes.
- S17 Camera (P4): shapes circle / rounded / square, 4 corners, size 10–40 %; mic and system audio
  record to separate files and mix at export with per-track gain and mute.
- S18 Transcript (P5): `Remove filler words` removes the configured list (`um, uh, eh, este, o sea,
  like, you know` by locale) and `Tighten pauses` shortens silences > 0.7 s to 0.25 s; both are
  ordinary EDL trims (undoable). Transcription runs on-device only; when unavailable the tab says
  `On-device transcription is not available for {language} on this Mac.`
- S19 P6 effects (loupe, split, cutout, looks, face tracking) are EDL fields, optional, and a
  project without them renders exactly as before (golden-frame test).
- S20 Docs EN/ES, settings docs, settings index and CHANGELOG describe each shipped phase;
  `npm run gen:docs` leaves no diff; the `description` is 120–155 characters in both languages.
- S21 The helper obeys the JSON protocol above and exit codes; no text or path in argv; stdout holds
  only protocol lines.

## Tasks (work units, ~1 commit and one PR each, ≤ ~400 authored lines)

Test-first on pure core logic: write the failing test, run it and record RED, implement, record
GREEN. Swift has no unit tests; its evidence is `scripts/capture-smoke.sh` output. "Lines" counts
authored lines (excluding generated docs output and fixtures).

### Phase 0, foundations

| ID | Specs | Work | Files | Test plan | Lines |
| --- | --- | --- | --- | --- | --- |
| T1 | S2 | `capture` module skeleton, `CaptureSettings` + defaults, `Settings.capture` | `crates/core/src/{lib.rs,models.rs,capture/mod.rs}` | old JSON loads; round trip; defaults per S2 | 150 |
| T2 | S21 | `capture::protocol` types + helper skeleton (`hello`, `shareable`, `unsupported-os`) + `macos/src/capture.rs` client with timeouts, `os_at_least` | `crates/core/src/capture/protocol.rs`, `crates/macos/src/{capture.rs,lib.rs,system.rs}`, `crates/macos/swift/capture/{main,Protocol,Shareable}.swift` | protocol round trips (RED/GREEN); client against a fake helper script (`/bin/sh` echo server) for framing, timeout kill, stderr logging | 400 |
| T3 | S3 S4 S21 | bundle build + sign + Info.plist strings + entitlements + `capture-smoke.sh` + CI assertion + formula caveat. **Own commit, risk item 5** | `scripts/bundle.sh`, `scripts/capture-smoke.sh`, `resources/Ferrocast.entitlements`, `.github/workflows/ci.yml`, `Formula/ferrocast.rb` | run `scripts/bundle.sh`, `codesign --verify --strict --deep`, `otool -L` shows weak ScreenCaptureKit, smoke script, `scripts/tests/release_dry_run.sh` | 150 |

### Phase 1, screenshot suite

| ID | Specs | Work | Files | Test plan | Lines |
| --- | --- | --- | --- | --- | --- |
| T4 | S5 S6 | `capture::region` pure geometry: normalise drag rect, clamp to display, window snapping, top-left↔Cocoa flips across mixed Retina/non-Retina and negative-origin displays; file naming with collision | `crates/core/src/capture/region.rs` | RED/GREEN: 2-display layouts incl. display above primary; rect crossing displays is clipped to the one with the pointer | 300 |
| T5 | S5 | Selection overlay `NSPanel`s, crosshair, dim hole, size label, Space/Esc | `crates/macos/src/{selector.rs,lib.rs}` | pure layout tests like `glow.rs` `layout_for`; manual run on 2 displays | 400 |
| T6 | S6 S4 | Helper `screenshot` (display/window/rect via `SCScreenshotManager.captureImage`), PNG write, Rust wrapper, permission mapping | `crates/macos/swift/capture/Screenshot.swift`, `crates/macos/src/capture.rs` | smoke script takes a screenshot when permitted; fake-helper test for the `permission-screen` mapping | 300 |
| T7 | S1 S3 S4 S6 | Builtins `Capture Area/Window/Screen`, settings gating, shortcuts, toasts, save folder + naming, after-capture routing | `crates/app/src/{items.rs,app.rs,services.rs}` | `tests.rs`: items present/absent by availability; gating toast once per session; routing by `after` | 350 |
| T8 | S7 | `View::CaptureResult`: preview, Copy, Save, Annotate…, Background… buttons, shortcuts | `crates/app/src/{app.rs,draw.rs}` | render test in `tests.rs` (as for Annotate); Annotate/Background act on the saved file | 350 |
| T9 | S2 S20 | **Settings → Capture tab** (rows: folder, format, after, cursor, fps, countdown, shortcuts) + index renumbering. Isolated: touches shared constants | `crates/app/src/draw.rs`, `crates/core/src/settings_index.rs`, `crates/app/src/tests.rs` | `sd3_every_drawn_row_is_indexed_and_every_entry_is_drawn` updated and green; keyboard nav `⌘1…⌘8` | 300 |
| T10 | S8 | OCR: enable `VNRecognizeTextRequest` features, `vision::recognize_text`, `capture::ocr` ordering, `Act::CopyText`, Copy Text button | `crates/macos/{Cargo.toml,src/vision.rs}`, `crates/core/src/capture/ocr.rs`, app | core ordering tests RED/GREEN; Vision test skipped on "inference context" (as `vision.rs:57-60`) | 300 |
| T11 | S9 | Pin panel | `crates/macos/src/pin.rs`, app | pure zoom/clamp tests; manual | 300 |
| T12 | S10 | `capture::stitch` overlap detection + merge (row-hash prefilter, SAD verify, sticky-header/footer detection) | `crates/core/src/capture/stitch.rs` | RED/GREEN on synthetic RGBA scrolled strips: exact overlap, 1-row step, fixed header, end-of-scroll duplicate, no-overlap fallback (reject) | 350 |
| T13 | S10 | Capture Scrolling driver: post `CGEventCreateScrollWheelEvent2` over the area, capture strip per step, stop rules, progress toast | `crates/macos/src/capture.rs` (scroll), `crates/app/src/{items.rs,app.rs}` | stop-rule logic pure (core); manual on Safari and Finder | 300 |
| T14 | S20 | P1 docs EN/ES, settings docs, CHANGELOG | `web/src/content/docs/**/screen-capture.md`, `…/reference/settings.md`, `CHANGELOG.md` | `npm run gen:docs` diff clean; description length 120–155 | 250 |

P1 ships after T14.

### Phase 2, recording

| ID | Specs | Work | Files | Test plan | Lines |
| --- | --- | --- | --- | --- | --- |
| T15 | S11 | `capture::events` + `capture::project` (create, load, save atomically, `validate` skeleton, thumb path) | `crates/core/src/capture/{events,project}.rs` | RED/GREEN: decode, resample, idle spans, unknown fields preserved, newer `version` rejected | 400 |
| T16 | S11 | Helper `record.start/stop/pause/resume`, SCStream → AVAssetWriter, `tick`/`warn` events, App Nap | `crates/macos/swift/capture/Recorder.swift` | smoke: 2 s recording duration within 0.1 s of wall time, static screen included | 400 |
| T17 | S11 S12 | Helper `Events.swift` (60 Hz cursor, clicks, scroll, window frames, key tap when permitted) + cursor kind detection spike | `crates/macos/swift/capture/Events.swift` | smoke: `events.json` has ≥ 55 cursor samples/s while the mouse moves (script moves the mouse via `cliclick`-free `CGEventPost` in a tiny Swift helper in `scripts/`); decide cursor-kind fallback | 300 |
| T18 | S13 | Recording HUD panel, countdown, tray recording state, Stop in tray | `crates/macos/src/hud.rs`, `crates/app/src/services.rs`, `app.rs` | pure layout/time-format tests; manual | 350 |
| T19 | S1 S11 S13 | Builtins Record Screen/Area/Window, Open Last Recording, Recordings browser view, disk-space guard, thumbnails | `crates/app/src/{items.rs,app.rs,draw.rs}`, `crates/core/src/capture/project.rs` | list/sort/rename pure tests; disk guard threshold tests | 400 |
| T20 | S20 | P2 docs + CHANGELOG | docs, CHANGELOG | gen:docs clean | 150 |

### Phase 3, editor

| ID | Specs | Work | Files | Test plan | Lines |
| --- | --- | --- | --- | --- | --- |
| T21 | S14 | `capture::curves` + `capture::zoom` (auto zooms, viewport follow) | `crates/core/src/capture/{curves,zoom}.rs` | RED/GREEN golden: click clusters, merge, lead/idle, scale clamp, viewport inside display, no overshoot of the spring (property test over 1000 random inputs with a fixed seed) | 400 |
| T22 | S14 | `capture::cursor` smoothing, idle hide, `blur_params` | `crates/core/src/capture/cursor.rs` | RED/GREEN: click timestamps preserved, lag bound ≤ 1 frame, blur 0 under threshold | 300 |
| T23 | S15 | `capture::timeline` mapping + EDL validation + `Edit` undo stack | `crates/core/src/capture/{timeline,project}.rs` | RED/GREEN: trims+speeds mapping round trip, overlaps rejected, duration math, undo/redo | 400 |
| T24 | S15 | Helper `render.frame`: background/padding/radius/shadow/aspect (no zoom yet), cache, PNG out | `crates/macos/swift/capture/Render.swift` | smoke renders fixture frame; dimensions asserted; golden hash tolerance via mean pixel diff | 400 |
| T25 | S15 | Render camera transform (zoom viewport), cursor sprites, click ring, motion blur | `Render.swift` | smoke golden frames at 3 times; perf budget printed (≤ 45 ms at 960 px) | 400 |
| T26 | S15 | Editor view: preview, scrubbing coalescing, play/pause, timeline widget, open from Recordings | `crates/app/src/{app.rs,draw.rs}` | render test with a fake client; coalescing logic pure | 400 |
| T27 | S15 | Editor panels: Zoom list/edit, Cursor, Style, trims/split/speed, undo/redo, autosave | `crates/app/src/draw.rs` | EDL edits via pure functions tested in T23; UI render tests | 400 |
| T28 | S16 | Helper `render` MP4 export + progress/cancel + audio passthrough | `Render.swift`, `Export.swift`, app `Export` tab | smoke exports fixture, `ffprobe`-free check with `AVAsset` in the smoke Swift; cancel leaves no file | 400 |
| T29 | S16 | GIF export | `Export.swift` | smoke: GIF ≤ 960 px, frame count = 15 × duration ± 1 | 200 |
| T30 | S20 | P3 docs + CHANGELOG | docs | gen:docs clean | 250 |

### Phase 4, camera and audio

| ID | Specs | Work | Lines |
| --- | --- | --- | --- |
| T31 | S17 | Helper `devices`, `Camera.swift` (AVCaptureSession → `camera.mov`), `Audio.swift` (mic + system audio to m4a), source offsets in `project.json`, permission prompts and S12 toasts | 400 |
| T32 | S17 | Camera layout in core (`camera` EDL, shapes, corners, sizes, validation) + render in `Render.swift` | 350 |
| T33 | S17 | Editor Camera/Audio panels, audio mix in export with speed map, Settings → Capture device pickers | 400 |
| T34 | S20 | P4 docs + CHANGELOG | 150 |

### Phase 5, transcript

| ID | Specs | Work | Lines |
| --- | --- | --- | --- |
| T35 | S18 | `capture::transcript` (filler lists per locale, pause tightening, word range → trims) test-first | 350 |
| T36 | S18 | Helper `Transcribe.swift`: SpeechAnalyzer (`#available(macOS 26)`, compiled only with SDK ≥ 26) and `SFSpeechRecognizer` with `requiresOnDeviceRecognition = true` (14/15); availability message | 400 |
| T37 | S18 | Editor Transcript tab: word list, select/delete, Remove filler words, Tighten pauses; docs + CHANGELOG | 400 |

### Phase 6, extras

| ID | Specs | Work | Lines |
| --- | --- | --- | --- |
| T38 | S19 | Glass loupe (EDL `loupes`, render effect, timeline track) | 350 |
| T39 | S19 | Split-screen layout | 250 |
| T40 | S19 | `capture::lut` `.cube` parser + Looks (CIColorCube) + 6 bundled looks generated in code | 350 |
| T41 | S19 | Camera cutout (Vision person segmentation, beta flag) | 300 |
| T42 | S19 | Face tracking (`analyze.face` → `camera_track.json`, reframing maths in core) | 400 |
| T43 | S20 | P5/P6 docs + CHANGELOG | 200 |

### Rules for every task (Sonnet, read before T1)

1. Worktree: never work in the main checkout (it is on `feat/welcome-intro` with other sessions'
   uncommitted work). Create your own:
   `git worktree add target/wt-capture -b feat/screen-capture origin/master` (`target/` is
   gitignored) and work only inside `target/wt-capture`.
2. Build dir: always `export CARGO_TARGET_DIR=<repo>/target/wt-capture-target`; a shared target dir
   produced stale builds before.
3. Never `git add -A` or `git commit -a`; stage only the paths of the task (`git add <paths>`),
   check `git diff --cached --stat` before committing. Rebase on `origin/master` before each push;
   push fast-forward only; never force-push, never touch other branches.
4. Chained PRs: one PR per task (~400 authored lines max; split otherwise), each based on the previous
   task's branch (`feat/screen-capture-t7` → …), tests and docs with the code. Use the chained-pr and
   work-unit-commits skills.
5. Checks before every commit (all must pass):
   `cargo fmt --check`
   `cargo clippy --workspace --all-targets --locked -- -D warnings`
   `cargo test --workspace --locked`
   and the CI-like run with a fake HOME:
   `CARGO_HOME=/Users/drumstock/.cargo RUSTUP_HOME=/Users/drumstock/.rustup HOME=/tmp/fakehome PATH="/Users/drumstock/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin" cargo test --workspace --locked`.
   For tasks with Swift: also `scripts/capture-smoke.sh`. Tests must not need Screen Recording,
   camera or a GPU; permission-dependent checks live in the smoke script and skip when ungranted.
6. Keep `Cargo.lock` changes to the strictly needed lines and `--locked` green; add no new crates in
   P1–P3 (JSON via existing `serde_json`, PNG via the existing image stack). Ask before adding any.
7. Re-grep each quoted line reference before editing: other agents change `app.rs`, `draw.rs`,
   `items.rs` all the time. Make additive edits (new match arms, new variants at the end of enums)
   to minimise conflicts.

## Spike results (2026-10-23, this Mac: macOS 27.0 build 26A428, SDK macOS 27.0 per
`xcrun --show-sdk-version`, arm64)

- `swiftc`: `swift-driver version: 1.168.6 Apple Swift version 6.4 (swiftlang-6.4.0.34.1
  clang-2100.3.34.1)`, target `arm64-apple-macosx27.0.0`. (The translation plan wrote "SDK macOS 26";
  `xcrun --show-sdk-version` prints 27.0 today: check again before relying on SDK-26 gating.)
- Probe: `target/capture-spike/main.swift` (gitignored), built with
  `xcrun swiftc -O -target arm64-apple-macos13.0 -Xlinker -weak_framework -Xlinker ScreenCaptureKit main.swift -o probe`.
  Top-level code requires the file be named `main.swift` and **no** `-parse-as-library`
  (first attempt with `-parse-as-library` failed: "expressions are not allowed at the top level").
- `otool -L probe` lists ScreenCaptureKit with the `weak` flag: the binary loads on older macOS.
- Run output: `preflight: false` (`CGPreflightScreenCaptureAccess`), `SCRecordingOutput available:
  true` (macOS 15 API, present here), `captureMicrophone available (default): false` (property
  exists on `SCStreamConfiguration` and defaults to off), `SCScreenshotManager available: true`.
- `SCShareableContent.getExcludingDesktopWindows` failed without permission with
  `com.apple.ScreenCaptureKit.SCStreamErrorDomain -3801`, message (system locale is Spanish):
  `El usuario ha rechazado las preferencias de TCC para la captura de aplicaciones, ventanas o pantallas`.
  So the display count could **not** be obtained from a bare CLI: the probe has no TCC grant. Whether
  the grant carries over when the helper is launched from the signed Ferrocast bundle is untested
  (Risks, T3).
- Not tested: actual capture, `SCStream` audio/mic on macOS 14, `NSCursor.currentSystem` kinds,
  SpeechAnalyzer, Metal availability on CI.

## Decisions to confirm before T1

1. Preview playback by frame requests (`render.frame` + prefetch) instead of an `AVPlayer` with a
   Core Image video composition. The second gives smoother playback but couples preview and export
   pipelines to AVFoundation compositing and cannot do the same motion blur. Plan assumes frame
   requests; revisit if S15 (≥ 24 fps) fails on T26.
2. OCR in Rust via `objc2-vision` (new feature flags) rather than the helper. Plan assumes Rust so
   Copy Text needs no helper round trip.
3. CI/release runner: stay on `macos-14` until P5 needs SDK 26 for SpeechAnalyzer, or move to
   `macos-26` together with the translation plan (one runner change for both).
4. Default storage: `~/Pictures/Ferrocast Captures` and `~/Movies/Ferrocast Recordings`, not the app
   data folder (`storage::Store::default_location`, `crates/core/src/storage.rs:15`).
5. No default global shortcuts (avoids clashing with ⇧⌘3/4/5); the user opts in.
6. Settings tab position and group: Capture after Translate in the "Intelligence" group, or as a
   fourth group before Power. Plan assumes the latter (`SETTINGS_GROUP_STARTS = [3, 5, 6]`).
7. The Screen Studio inventory is from public knowledge, not from running the product; confirm the
   rows before P6.
8. Key presses are recorded only as shortcuts (never plain typing). Confirm this privacy rule.

## Risks

- **Permissions UX**: Screen Recording shows a system prompt on first use and often requires an app
  restart on older macOS; the helper-versus-app TCC row is unverified. Mitigation: T3 smoke from the
  signed bundle, clear S4 toast, never leave partial files.
- **Item 5 (delivery/CI)**: new signed binary in the bundle, new Info.plist keys, possible runner
  change; T3 gets its own commit and review. Ad-hoc signature makes macOS re-ask permissions after
  each rebuild (already noted in `bundle.sh:50`); worse for Screen Recording, document it.
- **Performance**: 60 fps capture of a 3456×2234 display plus preview rendering at once; encoder
  hardware must keep up. Mitigations: HEVC 420v capture, `queueDepth` 6, preview paused while
  recording, dropped-frame counter in `tick`, 30 fps option. 4K60 export time can exceed real time
  on older chips; progress shows ETA.
- **Disk usage**: raw HEVC at 60 fps is ~1–3 GB per 10 min; the S13 guard, project size shown in
  the browser, and "Move to Trash" in Recordings.
- **App Nap and display sleep** stop capture mid-recording: `beginActivity` + IOPM assertion in T16;
  verify with a 5-minute run.
- **Multi-display and Retina**: points vs pixels, negative display origins, mixed scales, windows
  on several displays; T4 tests the maths, T6/T16 verify on this Mac with the external display.
- **Metal availability**: virtual CI machines may lack a GPU (Vision already fails there,
  `vision.rs:57-60`); `hello.metal=false` makes smoke tests skip rendering gracefully, the CPU path is
  slow but correct.
- **Cursor shape detection** (`NSCursor.currentSystem`) is unverified; fallback is an arrow-only
  cursor, which is acceptable for P3.
- **On-device transcription availability**: `SpeechAnalyzer` is macOS 26+ and needs the language
  model downloaded; `SFSpeechRecognizer` on 14/15 supports on-device recognition for a limited
  language list and may cap audio length (assume ~1 min chunks: chunk in the helper). Without either,
  the Transcript tab says so (S18); no cloud fallback.
- **Scrolling capture** depends on Accessibility to post events and on apps honouring synthetic
  scrolls; sticky headers and lazy-loaded content can break stitching, so T12 rejects low-confidence
  overlaps and the UI offers the partial result.
- **Scope**: 43 tasks is a long chain; each phase must stay shippable. If P3 slips, P1 and P2 still
  ship (recordings open in Finder).
- **Concurrency** (below): conflicts in `app.rs`/`draw.rs`/`items.rs` are the main schedule risk.
- Unverified (see list in the final report of the planning task): TCC attribution of the helper,
  `SCStream` details on macOS 14, cursor kinds, `macos-14` runner SDK for the helper build.

## Concurrency with other agents

- Other sessions work in the main checkout on `feat/welcome-intro` with uncommitted changes
  (`odd/plans/apple-translation-plan.md` is itself untracked there). Do not stage, stash, checkout,
  reset or commit anything in the main checkout; do all work in `target/wt-capture` (Rules 1–3).
- Hot files shared with others: `crates/app/src/{app,draw,items,services,tests}.rs`,
  `crates/core/src/{models,settings_index}.rs`, `crates/macos/src/lib.rs`, `scripts/bundle.sh`,
  `.github/workflows/ci.yml`, `CHANGELOG.md`. Keep edits additive and small, rebase before each
  push, and resolve CHANGELOG conflicts by keeping both bullets.
- The translation plan (`ferrocast-translate`) touches the same build script, the same
  `os_at_least` helper and possibly the CI runner. Coordinate: the first to land adds
  `build_swift_helper` and `os_at_least`; the second reuses them and must not duplicate. Check
  `git log origin/master -- scripts/bundle.sh` before T3 and before T2.
- T9 (settings tab renumbering) is deliberately a separate PR so a conflict there never blocks the
  feature work; rebase it last in the chain.
- Use only your own `CARGO_TARGET_DIR`; never run `cargo clean` or touch `target/` outside
  `wt-capture*` and `capture-spike`.
