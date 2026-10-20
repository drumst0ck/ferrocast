<!-- Produced by claude-fable-5-1 via gentle-ai-explore (read-only), task mvgt2zws-l-0jo1. Evidence marked UNVERIFIED was not checked. To be executed by Sonnet. -->

# Ferrocast × Tangerine parity, wheel redesign and website: execution plan

Planner output, read-only. I did not edit files. Evidence is cited as `path:line`; anything I did not observe is marked **UNVERIFIED**.

---

## 1. Executive summary and honest current state

**What exists, in code.**
- The conversion matrix is encoded in `crates/core/src/matrix.rs` (46 inputs, 275 options, unit-tested at `matrix.rs:~75`).
- All 275 pairs execute on real fixtures, per the log (L12, L17, `crates/app/src/matrix_test.rs`). The check is non-empty output with a content read-back. It does not prove layout fidelity.
- Windows exist for: annotate, photo edit, crop, redact, background, collage, PDF organizer, and five video windows (trim, crop, speed, snapshots, redact).
- Typed commands exist for compress by size, percent and preset, metadata (JPEG make/model/gps), trim, split, merge, normalize, and others.
- The drag wheel has three kinds (`WheelKind` at `crates/core/src/wheel.rs:6`) and a tested state machine (`Tracker`).

**What is missing or only approximate.** Audit result:

| # | Area | Tangerine (S10/S11) | Ferrocast now (evidence) | Status |
|---|---|---|---|---|
| G1 | Tool entries per format (`et()` mapping) | Fixed list per format, multi-select variants | No `tools_for(ext, multi)` exists. Entries come from `ImageOp::offered` (`convert.rs:696`) for images and `FileTool::offered` (`convert.rs:1081`) for audio/video/PDF. Archives and gif get no tool list. No Compress, Edit-metadata or Extract entry for audio/video/pdf. | Approximate |
| G1b | Tools wheel for non-image files | Per-format tools | Fixed 8 image tools, otherwise one "Open tools…" segment (`app.rs:977-1008`) | Missing |
| G2 | Actions wheel | Exactly 6, in order AirDrop, Float, Copy, Move, Copy Path, Terminal | 8 segments, adds Quick Look and Convert…, different order (`app.rs:962-975`) | Approximate |
| G3 | Convert wheel | All outputs shown | `.take(8)` and `needs.is_none()` filter (`app.rs:954-958`). PDF, textClipping and docs have 9 outputs, so one is dropped. Tool-dependent targets are hidden, not hinted. One identical icon per family (`app.rs:947-953`). | Approximate |
| G4 | Enter on a Finder selection | Plain Return opens the format menu | Only the command "Convert Finder Selection" (`app.rs:2883`) and ⌥↵ on a result (`app.rs:2322`). `keyboard.rs:14` only posts Return. The only tap is listen-only (`expansion.rs:69`). | Not implemented (by decision, L58-L59) |
| G5 | Compress window (target size, percent, presets, resize, live estimate) | Window with estimate; for image, audio, video, PDF | Images only, typed (`parse_image_query`, `convert.rs:756`, `services.rs:1944`) plus 3 fixed presets. No window, no estimate. No audio, video or PDF compress anywhere (grep "compress" finds only image code). | Missing / typed |
| G6 | Edit metadata window | Inspect, change, remove; fields grouped by file/track/chapter | Typed `make/model/gps` for JPEG only; removal JPEG/PNG only (`services.rs:2321` error text). No window. Nothing for audio, video, gif or PDF. | Missing |
| G7 | mergePDF, createPDF, join, merge audio, collage order | Finder order, reorderable | `FileTool::Merge`/`Join` and `ImageOp::CombinePdf` run in list order with no reorder UI. No drag reorder in collage or the PDF organizer (README, L45). | Missing |
| G8 | splitVideo, splitPDF | Custom points or equal parts, all clips in one folder | `SplitEvery(secs)` only (`convert.rs:1091`); `SplitPages` only for PDF. No point or equal-part UI. | Approximate |
| G9 | Audio tools | trimAudio waveform with silence removal and exact times; normalize with compare-levels view; channels window; bleep window; visualizer (landscape, portrait, square, still image) | Presets or typed commands. `TrimSilence`, `Channels`, `Bleep`, `Normalize` and `Visualize` are all preset or typed. The normalize toast shows before/after LUFS (L22). No waveform or windows. Portrait is not offered (`offered`, `convert.rs:1099-1100`). | Missing UI |
| G10 | redactVideo | Several areas, per-redaction time range | One area; reference frame fixed at 10 % (`app.rs:3256`, `3280`) | Approximate |
| G11 | Image extras | Another image as background; adjustable blur; drag reorder | Background image is typed-only. Redact blur is fixed at 24 px, and clicking empty space doesn't deselect (L46). | Approximate |
| G12 | extractArchive | Tool for zip/tar/gz/rar | Only conversion via `route.rs:180-205` and `services.rs:1546` (unrar). No Extract action. RAR proven only on a hand-built fixture, because the only `unrar` is x86 (L12, README). | Missing / unproven |
| G13 | Pages.app compatibility | `.pages` opens in Pages | `.pages` is written in iWork '09 style and opened only with LibreOffice (README) | Unproven |
| G14 | Interaction proof | n/a | Keys and mouse unproven in a real session for the wheel, organizer, canvas, sliders, drag and video windows. L41 B1 has no regression test. | Unproven |
| G15 | Advisories | n/a | L23/L25/L26/L41/L46/L56: Vision has no deadline, `image_size` on the UI thread, WMV mute/crop fails (README), `for_pixels` memory, and others | Open debt |

**Not verified by me:** that any of these windows work interactively, the real native behaviour of the shelf, AirDrop and Terminal, and egui 0.36 API details (`dnd_drag_source`, `Mesh`). Check each in the first unit that uses it.

---

## 2. PART A: parity units

### Common rules for every unit
- Write RED tests first and record the failing exit code (101 for compile or test failure).
- Then implement, and re-run until green.
- Closing checks, three separate commands, each exit code recorded individually and never piped:
  1. `cargo fmt --check; echo "fmt=$?"`
  2. `cargo clippy --workspace --all-targets -- -D warnings; echo "clippy=$?"`
  3. `cargo test --workspace; echo "test=$?"`
- Outputs never overwrite originals (use `output_with_suffix`/`output_path` at `convert.rs:356`, `convert.rs:733`).
- No `panic=abort`, no per-frame heavy work, and no process spawned on the UI thread.
- Commit tag: `<unit-id>: <what>`.
- "Model-verifiable" means checks via tests or screenshots only. "Human" means it needs a real session (see §7).

### A1. Core `tools_for` mapping (G1) — S, model-verifiable
- **Tangerine:** the `et()` mapping in S11. Per format: jpg/png/webp/heic/tiff get 7 tools, with 5 for multi-select. svg gets only multi: createPDF and createCollage. mp3/m4a/wav/flac get 7. mp4/mov/mkv get 9, with 7 for multi. gif gets removeMetadata. pdf gets 4. archives get extractArchive.
- **Now:** no such mapping exists.
- **Change:** new `crates/core/src/tools.rs`.
  - `enum ToolId` with 26 variants: Compress, RemoveMetadata, EditImage, AnnotateImage, FrameImage, CropImage, RedactImage, CreateCollage, TrimVideo, CropVideo, ChangeVideoSpeed, JoinVideos, VideoSnapshots, SplitVideo, RedactVideo, NormalizeAudio, AudioToVideo, TrimAudio, AudioChannels, RedactAudio, MergePdf, OrganizePdf, MuteVideo, SplitPdf, CreatePdf, ExtractArchive.
  - `pub fn tools_for(ext: &str, multi: bool) -> Vec<ToolId>`, plus `pub const ACTIONS: [ActionId; 6]` in order airDrop, float, copy, move, copyPath, openTerminal.
  - `pub fn implemented(t: ToolId) -> Impl { Window | Preset | Typed | Missing }`, an honest const table that A2 and C1 consume.
  - Register the module in `crates/core/src/lib.rs`.
- **Tests (RED):**
  - `tools_for("png", false).len()==7`; `("png", true)` is `[Compress, RemoveMetadata, CropImage, CreatePdf, CreateCollage]`.
  - `("svg", false)` is empty and `("svg", true)` has 2.
  - mp4 single has 9 and multi has 7; gif single is `[RemoveMetadata]`.
  - pdf single has 4 and multi has `[Compress, RemoveMetadata, SplitPdf, MergePdf]`.
  - zip, tar, gz and rar give `[ExtractArchive]`.
  - Every one of the 26 ids appears in at least one list.
  - Aliases: `jpeg`→jpg, `tif`→tiff.
- **Verification:** the three cargo checks.
- **Risk:** low. S10/S11 text has one ambiguity: whether `m4a`/`ogg`/`opus`/`aiff`/`wma` get audio tools. S11 lists only mp3/m4a/wav/flac, so follow that exactly and record the exception in a code comment.
- **Dependencies:** none. Everything else depends on it.

### A2. Use `tools_for` in the Convert view and the Tools wheel (G1, G1b) — M, model-verifiable
- **Now:** `app.rs:1951-2226` builds "Image tools" and "File tools" with `ImageOp::offered`/`FileTool::offered`. The wheel's Tools branch is at `app.rs:977`.
- **Change:**
  - Add `fn tool_rows(&self, files) -> Vec<(ToolId, Item)>` in `app.rs` that maps each `ToolId` to its existing `Act` window or preset.
  - Mapping: AnnotateImage→`Act::AnnotateCanvas`, EditImage→`Act::EditPhoto`, CropImage→`Act::CropImage`, RedactImage→`Act::RedactImage`, FrameImage→`Act::FrameImage`, CreateCollage→`Act::CollageImages`, TrimVideo, CropVideo, ChangeVideoSpeed and so on to the existing `Act` variants, and OrganizePdf→`Act::OrganizePdf`.
  - Tools with no window yet map to the current preset or typed behaviour (Compress to presets, RemoveMetadata to `StripMetadata`, MuteVideo to `FileTool::Mute`).
  - The Tools wheel uses `tools_for(...)` for every family. When n>8 it relies on B5.
  - Tools whose `implemented()==Missing` are listed with accessory "Not available yet". They are not hidden, so the 26-tool claim stays honest and a test locks it.
  - Keep typed commands (`500kb`, `trim`, `frame …`) as they are.
- **Tests (RED), in `crates/app/src/tests.rs`:**
  - For mp4, png, pdf, zip, gif, mp3 and a multi-png selection, the Convert view's "Tools" section titles match `tools_for` order.
  - `wheel_segments(Tools, [mp4])` has 9 segments (needs B5), and for `[a.zip]` it has 1.
  - A txt file has no tools section.
- **Verification:** the cargo checks plus a screenshot `FERROCAST_VIEW=wheel FERROCAST_WHEEL=tools FERROCAST_FILES=/tmp/fc-fx/demo.mp4`.
- **Risk:** M (the existing tests at `tests.rs:366` and `:335` must be kept green or updated deliberately).
- **Dependencies:** A1. B5 for n>8 rendering.

### A3. Actions wheel exactly six, in Tangerine order (G2) — S, model-verifiable
- **Now:** 8 segments (`app.rs:962-975`).
- **Change:** `wheel_segments(Actions)` returns AirDrop, Float, Copy, Move to…, Copy Path, Terminal in that order, built from `tools::ACTIONS`. Quick Look and Convert… stay in ⌘K only.
- **Tests (RED):** the labels vector equals the six in order, and n==6 for one file, many files and a folder.
- **Verification:** the cargo checks plus a screenshot of the actions wheel.
- **Risk:** low. Terminal for several files uses only `f[0]` (`app.rs:968`). Per S5, "open a folder, or a file's containing folder", that is acceptable.
- **Dependencies:** A1.

### A4. Convert wheel shows every option (G3) — S/M, model-verifiable
- **Now:** `.take(8)`, filter `needs.is_none()` (`app.rs:954-958`), same symbol for every segment (`app.rs:947-953`).
- **Change:**
  - Remove `take(8)`, so up to 9 (PDF, textClipping, docs) are shown.
  - Keep targets that need an optional tool, and mark them with a `dim: bool` flag plus a hint line ("Needs ffmpeg"). Add `dim` to `WheelSeg` (`app.rs:213`).
  - Choosing a dim segment toasts the existing install hint instead of failing silently.
  - Per-target symbols through a pure `target_symbol(ext)` in `app.rs` or `ui.rs`: image sf:photo, pdf sf:doc.richtext, mp3 sf:waveform, mp4 sf:film, gif sf:livephoto, txt sf:text.alignleft, xlsx sf:tablecells, and so on.
- **Tests (RED):**
  - `wheel_segments(Convert,[a.pdf])` has 9.
  - With `Tools{ffmpeg:None}` an mp4 file keeps `mov`/`mkv`/`webm`/`avi`/`wmv`/`gif`/`mp3` all present, with the dim flag set where `route_needs` is Some.
  - Symbols are unique per output ext for pdf.
- **Verification:** the cargo checks plus screenshots `FERROCAST_FILES=/tmp/fc-fx/doc.pdf` and an mp4.
- **Risk:** low.
- **Dependencies:** B1 and B5 for ≥9 segments. If A4 lands first, the 9-segment ring will look cramped, so schedule it after B5.

### A5a-c. Enter on a Finder selection (G4) — L overall, three units
Hard constraints (L58): never steal Return globally, never stall the event tap, and keep it opt-in.
- **A5a (S, model-verifiable): pure decision function.**
  - Add `crates/core/src/finder_enter.rs`: `pub fn should_intercept(frontmost_bundle: Option<&str>, focused_role: Option<&str>, mods_empty: bool, enabled: bool) -> bool`. It is true only for Finder front, role in {AXList, AXOutline, AXBrowser, AXScrollArea, AXImage}, no modifiers and the setting enabled. False for AXTextField, AXTextArea, AXSearchField, AXComboBox and unknown roles.
  - Tests (RED): the table above, `None` role gives false, and enabled=false gives false.
- **A5b (M, needs human): macOS tap.**
  - New `crates/macos/src/finder_enter.rs`: a default (not listen-only) `CGEventTap` at Session level for keyDown keycode `36` (`keyboard.rs:14` RETURN).
  - The callback does only O(1) work. It reads an `AtomicBool` "Finder frontmost" kept by an `NSWorkspace` activation observer, and reads the focused-element role through `AXUIElementCopyAttributeValue` on the system-wide element with a 50 ms messaging timeout.
  - It returns `Drop` only when `should_intercept` is true; otherwise it passes the event through.
  - On `TapDisabledByTimeout` / `ByUserInput` it re-enables the tap.
  - On drop it sends `Msg::FinderEnter` over the bus.
  - Structure like `expansion.rs:59-90` (copy its thread and run-loop setup).
  - Tests: none that run the tap. Only unit-test any extracted pure code (role mapping).
  - Document the risk (a stalled tap freezes input) and keep the callback free of locks and I/O.
- **A5c (S/M, needs human): wire-up.**
  - Add the setting `enter_on_finder: bool`, default **false**, in Settings → General (near `draw.rs:3333`).
  - `Msg::FinderEnter` handler: call `finder_selection()` (with its 15 s deadline, L61) off-thread.
  - If the selection is non-empty, open the Convert wheel via the existing `Act::OpenWheel` path.
  - If it is empty, re-post Return with `keyboard.rs` (post keycode 36) so Finder behaves normally.
  - Tests (RED): the handler with an empty selection issues a repost request. Make the repost a trait-injected closure or a recorded Msg so it can be asserted.
- **Risk:** H for A5b/A5c (system-wide input, Accessibility permission). **Dependencies:** A5a→A5b→A5c. Do not merge A5b/c without the human check H3.

### A6a-e. Compress (G5) — five units
- **A6a (S, model-verifiable): pure plan.** New `crates/core/src/compress.rs`:
  - `enum Goal { TargetBytes(u64), Percent(u8), Preset(Preset) }`, `struct Resize { max_edge: Option<u32>, percent: Option<u8> }`.
  - `audio_kbps(target_bytes, duration_s) -> Option<u32>` (clamped 32..=320, `None` when impossible), `video_kbps(target, duration, audio_kbps)`, and `estimate_bytes(orig, goal)` for Percent and Preset.
  - Tests (RED): known arithmetic (target 1 MB over 60 s gives about 133 kbps total), impossible-target `None`, percent bounds 1..=95, presets monotonic.
- **A6b (M, model-verifiable plus screenshot): image Compress window.**
  - New `View::CompressImage{files,size,goal,resize,est}`. Controls: segmented Size / Percent / Preset, a numeric field, a "Resize while compressing" max-edge field, and a live estimate.
  - Estimate: on 250 ms debounce, a worker encodes an 800 px downscale at the candidate quality and scales by the pixel ratio. It is labelled "≈" and replaced by the real size in the result toast. A newest-wins thread, like the frame extractor (`draw.rs:1678`).
  - Executor: `ImageOp::CompressWith{goal,max_edge}` reusing `compress_to_size` (`services.rs:1944`) plus the existing resize path.
  - Tests (RED):
    - Executor on a real PNG/JPEG fixture with a percent goal gives size ≤ the target, and a resize goal gives the expected width.
    - Original untouched, collision gives " 2".
    - View render test with the estimate `None` and `Some`.
    - Enter with an invalid field starts no job (follow the redact range test, L54).
  - Screenshot: add `Ok("compress")` in the screenshot dispatch (`draw.rs:108-358`).
- **A6c (M, model-verifiable, needs ffmpeg): audio and video compress.**
  - Executor `FileTool::Compress(goal, resize)` in `media_tools.rs` plus builders `media::compress_audio`/`compress_video` in `crates/core/src/media.rs` (pure arg lists).
  - Audio: libmp3lame, aac or libopus at `audio_kbps`. Video: libx264 `-b:v` for a target, CRF for a preset, `-vf scale=-2:min(ih\,N)` for resize, plus `-movflags +faststart`.
  - Tests (RED): the builder's arg lists as pure tests, in the style of `media.rs:236-295`. Real-media test in `media_tools_tests.rs` (gate on ffmpeg present like the others): the output exists, is smaller, and the duration matches within 0.2 s.
  - The window is the A6b view in "media" mode (disable resize for audio, no estimate beyond a bitrate calculation).
- **A6d (M, **spike first**): PDF compress.**
  - Options: ghostscript when installed (`-dPDFSETTINGS=/ebook`), otherwise the macOS Quartz filter "Reduce File Size" through PDFKit/Quartz.
  - **UNVERIFIED** that a Quartz filter can be applied headlessly through `objc2`. The first step is a spike test in `crates/macos/src/pdf.rs` that tries it on a generated multi-image PDF and reports sizes.
  - If infeasible, ship gs-optional with an install hint, exactly like ffmpeg and LibreOffice (S6), and record it in the log as partial.
  - Tests: real-PDF test (size not larger; page count preserved via the existing `pdf.rs` inspect).
- **A6e (S): wire entries.** Compress is added to `tools_for` rows (A2). The wheel Compress segment opens the window, no longer a fixed 60 %.
- **Dependencies:** A1, A2. **Risk:** A6d is H, the rest M. All are model-verifiable except real-session slider feel (H5).

### A7a-d. Edit metadata (G6) — four units
- **A7a (M, model-verifiable): pure model.**
  - New `crates/core/src/metaform.rs`: `struct Field{group: Group(File|Track|Chapter), key, value, editable, remove}`, plus `Group`.
  - `parse_ffprobe(json: &str) -> Vec<Field>` using `serde_json`. It reads format tags, per-stream tags and chapters.
  - `ffmpeg_args(input, edits, out)` produces `-map_metadata -1`/`-metadata k=v`/`-map_chapters -1`.
  - `exif_fields_from_sips(...)` for images, mapping `make`, `model`, `gps`.
  - Tests (RED): fixture JSON with 2 tracks and 2 chapters gives the expected groups and order. Removal args. Editing a key leaves others intact. Unicode values are carried through.
- **A7b (M, model-verifiable): executors.**
  - `FileTool::SetMeta(Vec<(String,String)>, remove_all)` for audio, video and gif via ffmpeg `-c copy`. PDF through `pdf.rs` (title, author, subject, keywords, using PDFKit document attributes through the same osascript route as the existing code). Real-file tests that read the tags back with ffprobe or PDFKit.
  - Extend image removal beyond JPEG/PNG: HEIC/TIFF/WebP through `sips -d`? **UNVERIFIED**. Decide via a spike test, or keep the clear error at `services.rs:2321`.
- **A7c (S): reader.** Off-thread `inspect_metadata(path) -> Vec<Field>`, with an `ffprobe` deadline and bounded stdout (see how `media_tools.rs:20` bounds metadata).
- **A7d (M, screenshot): window.**
  - `View::EditMetadata{file, fields, loading}`: grouped sections with a text field per row, a "remove" toggle, and "Remove all". Enter saves a copy.
  - Enter and Esc must not be consumed while a text field is focused (the L41 B1 class). Follow the `read_keys` exemption.
  - Add `Ok("meta")` to the screenshot dispatch.
  - Tests: a render test; an event test that Enter with a focused field does not save.
- **Dependencies:** A1, A2. **Risk:** M.

### A8a-c. Reorder list and drag reorder (G7) — three units
- **A8a (S, model-verifiable):** `crates/core/src/reorder.rs` with `move_item(v, from, to)`, `move_up/down`, and `finder_order(paths)` (a stable sort that keeps the incoming selection order, since the Finder selection order is already the pick order). Tests: edge moves, a single item, and out-of-range indices that don't panic.
- **A8b (M, model-verifiable plus screenshot): reorder window.**
  - `View::Reorder{purpose: MergePdf|CreatePdf|JoinVideos|MergeAudio, files}`: rows with up/down buttons, ⌘↑/⌘↓ keys, a duration or page count where cheap, and Enter running the existing `FileTool::Merge`/`Join`/`ImageOp::CombinePdf` with the reordered list.
  - Reuses the executors proven in L16. For Join the first clip is the canvas, so show a "Canvas: first clip" label.
  - Tests (RED): a render test; the reorder key event changes the order; Enter dispatches the order.
- **A8c (M): drag-to-reorder** (`ui.dnd_drag_source`/`dnd_drop_zone`; check availability in egui 0.36 first). Apply it to this list, to the collage picture order (`View::CollageImage` plus a `files` reorder), and to the PDF organizer thumbnails (`View::PdfOrganize`, `pdfplan.rs`).
  - Pure drop-index maths goes in `reorder.rs` and is tested. The gesture itself is a human check (H8).
- **Dependencies:** A1, A2. **Risk:** M (8c: L because three views).

### A9a-c. Split (G8)
- **A9a (S, model-verifiable):** extend `crates/core/src/videoplan.rs`: `equal_parts(duration, n) -> Vec<(f64,f64)>`, and `points_to_ranges(duration, points) -> Vec<(f64,f64)>` (dedupe, sort, clamp, drop pieces under 0.05 s). Tests: boundary points, duplicate points, n=1.
- **A9b (M):** `FileTool::SplitPoints(Vec<(f64,f64)>, exact: bool)`. All clips go into one new folder next to the source ("<name> split", with " 2" collision), re-encoded when exact and stream-copied otherwise. Real-media test: 10 s fixture, points 3 and 7 give 3 clips of the expected durations. The folder never overwrites.
- **A9c (M):** `View::VideoSplit` reusing the trim timeline (`draw_video_trim`, `draw.rs:1627`). `M` adds a marker at the cursor, Backspace removes the last, a field "Equal parts: N", and Enter splits. Entry in `tools_for` rows. Audio split uses the same logic without the preview.
- **splitPDF:** S11 gives no extra detail, so the current "one PDF per page" stays. Add a typed `pages 1-3,4-6` split-by-ranges only if cheap, and **do not claim parity beyond what S11 states**.
- **Dependencies:** A1, A2; shares the timeline with A12. **Risk:** M.

### A10a-g. Audio tools (G9)
- **A10a (S, model-verifiable):** `crates/core/src/waveform.rs`: `peaks(samples: &[i16], buckets) -> Vec<(i16,i16)>`, and `silence_bounds(samples, rate, threshold_db) -> (start, end)`. Tests: silence at the start only, at the end only, a full-scale square wave, an empty input.
- **A10b (M):** `macos`/`app` extractor: `ffmpeg -i in -ac 1 -ar 8000 -f s16le -` read with a bounded size cap (60 s deadline, byte cap), off-thread, newest-wins.
- **A10c (M, screenshot):** `View::AudioTrim`: waveform painted with `Shape::line_segment` per bucket (cap 800 lines), reusing `videoplan::Trim` for markers, steppers, exact start/end fields, and "Remove silence at start/end" toggles. Enter dispatches `FileTool::Trim`/`TrimSilence`. Needs the end-only silence semantics from P3a (the log says they were repaired; check `media.rs:192`).
- **A10d (M):** normalize window with loudness, true-peak and range controls, a before/after meter bar using the existing ebur128 measurement (L22), and a "compare levels" display of input versus target. Reuses `FileTool::Normalize`.
- **A10e (S):** channels window: Mono/Stereo toggle, L/R volume sliders → `FileTool::Channels(stereo,l,r)` (exists).
- **A10f (M):** bleep window: waveform selection with multiple ranges. This needs `FileTool::BleepMany(Vec<(f64,f64)>)` and a pure builder in `media.rs`, because today's `Bleep` takes one range.
- **A10g (S):** visualizer window: waveform/still image selector (file chooser as in the stamp tool), and Landscape/Portrait/Square (portrait exists in `media.rs:7-11` but is not offered in `offered`).
- **Dependencies:** A1, A2; A10c/f share A10a/b. **Risk:** M. **Honesty:** the ffmpeg-dependent results are model-verifiable on fixtures; how the UI feels is human (H9).

### A11-A12. Video redact and reference frame (G10)
- **A12a (S):** a pure `frame_time(duration, cursor) -> f64` in `videoplan.rs`, plus a "Reference frame" scrub bar on `View::VideoCrop` and `View::VideoRedact`. It reuses the extractor behind `Msg::VideoRedactReady` (`app.rs:1137`, `3256`, `3280`) so the 10 % hard-coding is replaced by the cursor. Test: a cursor of 0.0 returns 0.0, and a cursor past the end clamps.
- **A11a (M):** `crates/core/src/redactplan.rs`: `struct Redaction{rect,start,end,blur}`, validation (`0 ≤ start < end ≤ duration`, rect inside the display dims), and `media::redact_video_many(...)` building a filter chain. It must handle rotated sources via `display_dims` (L56). Tests: two overlapping redactions, a time gate with `enable='between(t,a,b)'`, a rect clamped to the frame.
- **A11b (M):** `FileTool::RedactVideoMany` plus a real-media test of two regions at different times (sample pixel colour inside and outside the windows).
- **A11c (M):** window: a list of redactions on the left (select, add, delete), the movable rect as now, and a per-item range. Tests: an add/delete event, and an invalid range still blocks Enter (L54 test).
- **Dependencies:** A12a→A11c. **Risk:** M. WMV stays failing until A-H (G17).

### A13. Image extras (G11) — S/M, model-verifiable
- Background window: "Use image…" through the existing stamp file chooser path (L37) feeding `frame::Backdrop::Image` (exists per L21).
- Redact blur radius slider (replace the fixed 24 px, L46).
- Empty-space click deselects in `RedactImage`.
- Tests: the frame option round-trips, blur radius reaches `RedactWith`, and a deselect event test.
- Chooser interaction is human (H6).

### A14. extractArchive (G12) — M, model-verifiable
- **Change:** `Act::ExtractArchive(PathBuf)` plus `tools::ExtractArchive`.
  - Output folder: a sibling named after the archive without the extension, with " 2" on collision, never merging into an existing folder.
  - zip: `ditto -x -k`. tar: `/usr/bin/tar -xf`. gz: `tar -xzf` for a tar.gz, otherwise `gunzip -c`.
  - rar: the existing unrar route, with a clear hint when missing.
  - Reject path-traversal entries. Check with `unzip -l`/`tar -tf` that no entry starts with `/` or contains `..`; **UNVERIFIED** whether `ditto`/`tar` already refuse them, so test it.
- **Tests (RED):** zip, tar and gz fixtures extract with correct contents. A malicious zip with a `../x` entry is refused, or extracted inside the folder only. The existing destination is never overwritten.
- **Risk:** M. **Dependencies:** A1.

### A15. Real RAR (G12) — S/spike, partly human
- Probe whether macOS `bsdtar` (`/usr/bin/tar`) reads RAR5 (libarchive). **UNVERIFIED.** If yes, use it as the primary path and keep unrar as a fallback. Add a test with a real `.rar` fixture checked into `crates/app/tests/fixtures/` (needs a human to create one with an arm64 or official RAR tool, so ask for that, H10).
- Without a real fixture, keep the README wording "RAR only with a synthetic file".

### A16. Pages.app compatibility (G13) — human-only
- Open the generated `.pages` (`crates/core/src/pages.rs`) in Pages (H11). **Do not change code before that result.** If Pages refuses it, log it, open a spike for a modern package layout, and keep the README claim as "opens in LibreOffice only".

### A-T1/A-T2. Event tests for the proof gaps (G14) — M each, model-verifiable
- **A-T1:** `render(&mut app, events)` already takes events (`tests.rs:83`). Add tests using synthetic `Event::Key` for:
  - the wheel (arrows step, Enter applies, Esc cancels, and modifier switching via `RawInput.modifiers` with the `wheel_mods` temp state at `draw.rs:1015`);
  - ⌥↵ opening the wheel;
  - the organizer keys (R, D, Delete, ⌘←/→, Enter, Esc);
  - the crop and EditPhoto W/H boxes keeping focus for Enter/Esc (the missing L41 B1 test);
  - canvas ⌘Z and Delete.
- **A-T2:** synthetic `PointerButton`/`PointerMoved` drag tests for crop (corner drag changes the rect), redact (add area), annotate (arrow shape added) and the timeline bar (seek).
- Both need only `render`, so no human is required. They reduce, but do not remove, the real-session items.

### A-H. Hardening (G15, G17) — S units, model-verifiable
Pick up by priority, each its own small commit with a regression test where possible:
- Vision call deadline.
- `image_size` off the UI thread with a deadline.
- Per-pixel allocation in `for_pixels`.
- WMV mute/crop (an ffmpeg wmv2 encode route with a real-media test).
- A TOCTOU fix by creating outputs with `create_new`.
- Stale PDF-thumbnail textures.
- Stamp-chooser thread stacking.

### Part A file touch map
- Core: `crates/core/src/{tools,compress,metaform,reorder,waveform,redactplan,finder_enter}.rs` (new); `convert.rs`, `media.rs`, `videoplan.rs`, `lib.rs`.
- App: `app.rs`, `draw.rs`, `items.rs`, `services.rs`, `media_tools.rs`, `tests.rs`.
- Macos: `finder_enter.rs` (new), `pdf.rs`, `keyboard.rs`.

---

## 3. PART B: modern drag wheel

### Current state (evidence)
- `draw_wheel` is at `draw.rs:1544-1585`.
- Constants: `WHEEL_INNER=58`, `WHEEL_OUTER=178` (`app.rs:3687-3688`). The window is 750×475 (`ui.rs:7-8`, transparent and undecorated at `main.rs:31-35`).
- The backdrop is a flat `pal.bg` disc at `WHEEL_OUTER+6`, hairline separators (`pal.sep`), hover is a 24-quad strip at accent α0.35, and the accent is hard-coded `(255,140,60)` (`draw.rs:1554`), unrelated to the app accent `(240,83,95)` (`ui.rs:37`).
- Icons are 22 px orange tiles for every segment (`draw.rs:1573`).
- The hub is a plain `pal.field` disc with "N file(s)" or the hovered label (`draw.rs:1577-1584`).
- There is no animation and no reduced-motion handling. The wheel requests a repaint every 16 ms (`draw.rs:1546`).
- I viewed `/tmp/p5_wheel_convert.png` and `/tmp/p5_wheel_tools.png`:
  - The Convert shot shows 7 segments with identical icons.
  - Labels reach the neighbours' arcs ("Remove metadata").
  - The ring is flat black with a white outline.
  - Whether the PNG has alpha is **UNVERIFIED**.
- The real window is a vibrancy-less transparent egui surface, so "blur" must be faked, or done natively in B7.

### Design: "Halo ring"

**Geometry (points, wheel centre `c`).**

| Element | Value |
|---|---|
| Ring outer radius `R_OUT` | 176 |
| Ring inner radius `R_IN` | 70 |
| Backdrop disc | `R_OUT + 10` = 186 (diameter 372 fits the 475 height with 51 pt margin) |
| Segment angular gap | 2.5° (each sector inset by 1.25° per side) |
| Sector radial inset | 2 (draw `R_IN+2 … R_OUT-2`) |
| Hub radius | `R_IN - 6` = 64 |
| Sector tessellation | 16 steps over a 45° sector, `max(6, ceil(16 * 45° / sector))` otherwise |
| Icon tile | 28×28, corner 8, centred at radius 120 (about 8 outside the ring's centre line) |
| Label | at radius 120, offset y +26, size 11.0, max width 84, ellipsis on overflow |

**Layers, back to front.**
1. **Soft shadow.** Three `p.circle_filled(c + vec2(0,6), 186 + k, black_alpha(a))` with `(k,a)` = (16,10), (10,14), (5,20) in dark and (16,6), (10,9), (5,12) in light. This is a faux Gaussian at 3 shapes.
2. **Backdrop disc.** `circle_filled(c, 186, fill)`. Dark: `from_rgba_unmultiplied(24,24,27,222)`. Light: `(250,250,251,226)`. This is a translucent look (alpha 222 against the 248 of `palette().bg`, so the desktop shows through slightly).
3. **Rim.** `circle_stroke(c, 185.5, Stroke::new(1.0, rim))`, rim dark `white_alpha(34)`, light `black_alpha(40)`. Add a top-lit arc: `Shape::line(points -150°..-30° at r=184.5, Stroke(1.0, white_alpha(46)/white_alpha(120) light))`.
4. **Ring track.** `circle_stroke(c, 123.0, Stroke::new(106.0, track))` (a thick stroke gives the annulus), track dark `white_alpha(8)`, light `black_alpha(6)`.
5. **Segments.** One `egui::Mesh` per hovered or selected segment (replace the 24 convex quads). The vertices go in inner/outer pairs, with a radial gradient: inner colour α 0.10, outer α 0.34 of the wheel accent. Idle segments draw only a radial hairline separator at the gap centre: `line_segment` width 1.0 in `sep` (dark `white_alpha(16)`, light `black_alpha(14)`).
6. **Hover glow.** Three arc strokes at `R_OUT - 1`, spanning the segment's angular range minus the gap, in the accent with widths/alpha (10, 18), (6, 32), (3, 70). These are `Shape::line` polylines.
7. **Selected (keyboard).** The hover fill plus a 1.5 px accent outline of the sector (`Shape::closed_line`), so keyboard focus is visible without the pointer.
8. **Icon tiles.** Use `ui::paint_icon` with the per-segment colour. Hover: tile scale 1.12, with the label colour moving from `pal.sub` to `pal.text`. Dim segments (A4) use alpha 0.45 and the hint "Needs ffmpeg" in the hub.
9. **Hub.** Disc radius 64:
   - fill dark `white_alpha(10)` / light `black_alpha(6)` and a 1 px `pal.border` stroke;
   - top: the wheel name chip "CONVERT"/"TOOLS"/"ACTIONS" at 9.5 pt in the wheel accent;
   - centre: the hovered label (13 pt, `pal.text`), or the idle summary line from `hub_text` ("1 file");
   - below: the secondary line ("PNG image · 2.4 MB", 10.5 pt, `pal.sub`);
   - bottom: three key-cap pips "⇧ ⌘ ⌃", 9 pt, the active one filled in the accent and the others `pal.sub`.

**Accent per wheel.** Take them from the app's existing glyph palette (`app.rs:783`, `805`):

| Wheel | Colour |
|---|---|
| Convert | orange `(255,140,60)` (the existing converter tiles, `app.rs:1953`) |
| Tools | blue `(86,194,255)` |
| Actions | green `(89,212,153)` |

On light backgrounds, multiply the accent by 0.92 for the hover fill.

**Palettes.** Add `WheelPalette` in `ui.rs`, built from `Palette` plus the numbers above, with `fn wheel_palette(dark: bool, accent: [u8;3]) -> WheelPalette`.

**8-segment layout, and more than 8.**
- Segment `i` is centred at `i·360°/n`, clockwise from 12 o'clock. This is the current `segment_at` convention at `wheel.rs:~99` and must stay.
- n≤8: icon plus label in the segment.
- 9≤n≤12: icon only, in a smaller tile (24), with the label shown in the hub on hover or selection. Rule: show the label in the segment only if the arc length at radius 120 ≥ 72 pt, which `arc_len(n, r)` decides.
- n>12 (not reachable today; the maximum is 9): the first 11 plus a "More…" segment, with Tab or the scroll wheel cycling pages (unit B8, deferred).

**Animation** (all durations through `ctx.animate_*_with_time`; easing via a pure `ease_out_cubic(t) = 1 - (1-t)^3` applied to the animation value):

| Event | Behaviour | Duration |
|---|---|---|
| Open | `scale 0.92→1.0`, `opacity 0→1`, applied to every radius and `ui.set_opacity` (existing launcher fade is 0.14 s, `draw.rs:1045`) | 140 ms |
| Hover in/out | per-segment value `("wh", i)`: fill alpha, tile scale 1→1.12, label colour | 100 ms |
| Switch wheel | incoming segments fade and scale 0.85→1 with a 12 ms per-segment stagger (max 96 ms), accent colour lerp, hub text cross-fade | 120 ms + stagger, hub 80 ms |
| Hub text swap | alpha cross-fade | 80 ms |
| Idle | repaint every 33 ms (30 fps), 16 ms only while an animation is in flight or the pointer moved | n/a |

**Reduced motion.** At `show_wheel` read `NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion` once (objc2-app-kit; **UNVERIFIED** that the feature flag is enabled for `NSWorkspace` in the app crate, so check `Cargo.toml`) and store a `reduce_motion: bool`. When true, all durations are 0, there is no scale or stagger, and hover is an instant fill.

**Native blur (optional, B7).** Add an `NSVisualEffectView` behind the egui layer, in `.hudWindow` or `.underWindowBackground` material and `.behindWindow` blending. Mask it with a circular `CALayer` (`cornerRadius=186`, frame centred on the wheel) so only the disc blurs. Reset when the launcher view returns. Risks: transparent-window interplay with eframe/winit and the window's native shadow. Make it a flag that defaults off until a human validates it (H12). The fallback is the translucent α≈222 disc.

### Painter calls summary
`circle_filled`, `circle_stroke` (the thick-stroke annulus), `Shape::line`/`closed_line` for arcs and outlines, `Mesh::colored_vertex` + `add_triangle` for sectors, `Painter::text` with `Align2::CENTER_CENTER`, `ui::paint_icon` for tiles, `Rect::from_center_size` scaled by the hover value.

### Tests and how to look at it
- **Pure geometry in core** (`crates/core/src/wheel.rs`, new items):
  - `struct Geometry{r_in, r_out, gap_deg}`.
  - `sector_points(i, n, geo, steps) -> Vec<[f32;2]>`.
  - `arc_len(n, r)`.
  - `labels_fit(n, r) -> bool`.
  - `ease_out_cubic`.
  - `hub_text(&[PathBuf]) -> (String, String)`.
  - `segment_center(i, n, r)`.
- Tests:
  - For n = 1..=12, `segment_at` at each `segment_center` returns i.
  - The first and last sector vertices match the angles ±gap/2.
  - Sector points all lie inside [r_in, r_out].
  - `labels_fit(8,120)` is true and `labels_fit(12,120)` is false (64 pt < 72).
  - `ease_out_cubic(0)=0`, `(1)=1`, and it is monotonic.
  - `hub_text` for a single png (`"1 file"`, `"PNG image"`), for 12 mixed files (`"12 files"`, `"Mixed"`), and for a folder.
- **Render tests** (`crates/app/src/tests.rs`, using `render`):
  - A wheel with n = 1, 8, 9, 12 and `sel` None or Some(last), in dark and light.
  - n=0 doesn't panic. `draw_wheel` currently divides by `n` at `draw.rs:1550`, so guard it.
  - Set the theme through `ctx.set_theme`.
- **Screenshots.**
  - Existing dispatch (`draw.rs:317-331`) uses `FERROCAST_VIEW=wheel` and `FERROCAST_WHEEL=convert|tools|actions`. Your example `FERROCAST_VIEW=wheel-convert` does not exist.
  - Add `FERROCAST_WHEEL_SEL=<n>` to set `sel` (hover cannot be simulated, because `mouse_top_left` reads the global pointer).
  - Add `FERROCAST_THEME=dark|light` if no way to force a theme exists (**UNVERIFIED**; check `Settings` and `Theme` at `app.rs:630`).
  - Command: `FERROCAST_DATA_DIR=/tmp/fc-data FERROCAST_FILES=/tmp/fc-fx/a.png FERROCAST_WHEEL=convert FERROCAST_WHEEL_SEL=2 scripts/shot.sh /tmp/wheel_convert_hover.png FERROCAST_VIEW=wheel`.
  - `scripts/shot.sh` takes the output path first and `KEY=VAL` arguments after, as `scripts/shot.sh:1-5` shows.
  - It needs `cargo build` first (`target/debug/ferrocast`).
  - Capture matrix: {convert png, convert pdf (9), convert mp4, tools png, tools mp4 (9), actions} × {dark, light}, with and without `SEL`.

### Units
- **B1 (S, model-verifiable): core geometry.** Everything in the "pure geometry" list, plus moving the radii constants out of `app.rs:3687` into `Geometry::DEFAULT`, with `wheel_hit` calling it. Tests as above.
- **B2 (M, ~300 lines, screenshot): static redraw.** `WheelPalette`, layers 1-9 without animation, mesh sectors, hub text, per-wheel accent, n=0 guard, and the screenshot env additions. Screenshots compared by eye, and model-verifiable via the render tests.
- **B3 (S/M):** hover/selected animation, glow arcs, tile scaling, dim segments, 33/16 ms repaint policy.
- **B4 (M):** open and switch animation, hub cross-fade, the reduced-motion flag.
- **B5 (S):** adaptive n 1-12 (icon-only mode, label-in-hub, ellipsis). This must land before A2 and A4 show 9 segments.
- **B6 (S, human eyes):** capture the matrix and attach the PNGs, and run the visual checklist (H13).
- **B7 (M, optional, human):** native vibrancy circle behind the ring.
- **B8 (S, deferred):** paging for n>12.
- **Order:** B1, B2, B5, B3, B4, B6, (B7).

---

## 4. PART C: website (not published, no publishing steps)

### Inspection facts
- Astro 5, `base:'/ferrocast'`, `site:'https://drumst0ck.github.io'` (`web/astro.config.mjs`), and `three` for `keys3d.js`.
- Pages: `index.astro`, `changelog.astro` (imports `../../../CHANGELOG.md`), `404.astro`. `web/src/components/` is **empty**. `web/public/` has `img/*.jpg` (ai, clipboard, emoji, home, panel, search, settings, snippets, tz, windows), `slot/*.jpg`, `video/`, `og.jpg`, `sitemap.xml` (static, 2 URLs), `robots.txt`. The directory `web/public/shots` mentioned in the brief **does not exist**, so screenshots go to `web/public/img/` with a `files-` prefix.
- `Base.astro` takes `{title, description}` only. It emits og:title/description/image and twitter card, with no canonical, og:url or JSON-LD.
- Header/footer are inlined in each page (`index.astro:85-89`, `changelog.astro:9-24`).
- `index.astro:38` already carries a "File tools" bento item with overly broad copy: "Compress, crop, frame, collage or redact photos, and trim, speed up or join clips with typed commands". The nav (`index.astro:87`) has no Files link, and FAQ "What is missing today?" ignores file tools.
- Design tokens: `site.css:3-13` (`--accent:#f0535f`, `--surface`, `--stage`, `--radius:10px`, `--font-display` 'Stack Sans Headline'); easing and duration tokens in `base.css:29-35`. Gradients, glow and 3D are allowed by the user.

### Data pipeline (C1)
- **Source of truth:** `crates/core/src/matrix.rs` (`inputs()`, `outputs_for()`), `convert.rs:296 label_for(ext)`, `convert.rs:342 route_needs(input, out, &Tools)`, and `tools.rs` (A1).
- **Generator:** a core test-style binary `crates/core/examples/site_data.rs` that writes `web/src/data/conversions.json` and `web/src/data/tools.json`.
- **conversions.json shape:**
  ```
  {"generatedFrom":"crates/core/src/matrix.rs","inputs":46,"conversions":275,
   "families":[{"id":"images","name":"Images","inputs":[{"ext":"jpg","label":"JPEG","outputs":[{"ext":"png","label":"PNG","needs":null|"ffmpeg"|"LibreOffice"|"cwebp"|"unrar"}]}]}]}
  ```
  Family assignment comes from a table of ext→family in the example (Images, Audio, Video, PDF, Documents, Slides, Spreadsheets, Subtitles, Archives). `needs` is derived by calling `route_needs` with an all-absent `Tools`; confirm `Tools` is constructible without a real probe (**UNVERIFIED**; check `convert.rs:77`).
- **Drift guard:** a core test `site_data_matches_matrix` compares the regenerated JSON string with the committed file, and fails with a hint "run `cargo run -p ferrocast-core --example site_data`". The Astro build then never needs cargo, and the data can't go stale silently. The tool list in `tools.json` comes from `tools_for` (A1) plus `implemented()`, so the website's status badges are generated from the same table the app uses.
- **Status badges.** A hand-written `web/src/data/status.ts` copied from the README table, using four honest levels:
  - `tested` = "Run on real files in tests",
  - `built` = "Implemented, not tried in a live session",
  - `typed` = "Typed command only",
  - `planned` = "Not included yet".

  All 275 pairs get `tested`, with a footnote that layout fidelity is unchecked and that RAR and `.pages` are limited (README row "Conversión de archivos"). Interactive windows are `built`. Finder Enter is `planned`. AirDrop and Terminal show "not tried in a live session". Add a "Status as of <date>" line. The data is manual and must be refreshed whenever the README table changes, so note the dependency in the file header.

### Units

**C1 (M): data generator.** `crates/core/examples/site_data.rs`, `web/src/data/{conversions,tools}.json`, the drift test. Verify: the three cargo checks plus `jq '.conversions' web/src/data/conversions.json` equals 275 and the sum of outputs equals 275. Depends on A1 for `tools.json` (an interim `tools.json` from the current `offered` lists would be wrong, so wait for A1). Model-verifiable.

**C2 (S): shared components.** `web/src/components/SiteHeader.astro`, `SiteFooter.astro`, `StatusBadge.astro`, `Section.astro`. Use them only in the new page and the new home section, so the diff to `index.astro` and `changelog.astro` stays small. Optionally refactor the two existing pages in one follow-up commit.

**C3 (M): home section "Convert & file tools".** Insert after `.feat` (after the section whose id is `features`, `index.astro:166`), id `files`:
- headline, a 3-key explainer (Shift = Convert, Command = Tools, Control = Actions), the counters "46 input formats · 275 conversions · 26 tools · 6 file actions", a status line, a wheel visual, and a CTA button "Explore all conversions" linking to `${base}/files/`.
- Add `Files` to the nav (`index.astro:87`).
- Update the bento item (`index.astro:38`) to the honest copy and link it.
- Add a FAQ entry "Are conversions really local?" (README: nothing is uploaded; optional ffmpeg/LibreOffice are used only if installed).
- Update the "What is missing today?" answer to mention "Enter on a Finder selection" only if still true when the site is written.

**C4 (M): interactive wheel explainer.** `web/src/components/WheelDemo.astro` plus `web/src/scripts/wheel-demo.js`, an inline SVG ring built from `tools.json` and `conversions.json` samples. Three tabs (Shift / ⌘ / ⌃) switch the ring, with the keyboard keys switching too (`keydown` for Shift/Meta/Control). It uses the Part B palette (same accent per wheel, 8 segments, hover glow via CSS `filter: drop-shadow`) and the existing easing tokens. `prefers-reduced-motion: reduce` disables the transitions. `role="img"` with an `aria-label`, plus a real `<ul>` fallback for no-JS. No screenshots needed for the wheel itself. This avoids a real-capture dependency and any transparency problems.

**C5 (M): conversion matrix explorer on `/files`.**
- Build-time HTML for all 275 pairs, grouped in `<details open>` per family, rows "JPEG → PNG WebP …" with chips. SEO-friendly and usable with no JS.
- A client filter (`web/src/scripts/matrix-filter.js`): a text box (matches ext and label), family chips, a "from" `<select>`, a "to" `<select>`, a "needs nothing extra" checkbox, an `aria-live` result count, and `#q=` hash state.
- A `needs` marker on chips that require ffmpeg, LibreOffice, cwebp or unrar.
- The counts shown are computed from the JSON, and a unit assertion in the page frontmatter throws at build time if the total ≠ 275.

**C6 (M): tools section on `/files`.** Cards grouped as Image editors, Video windows, PDF organizer, Audio tools, Archives, each with a `StatusBadge`, the shortcut (from the README: ⇧⌘A annotate, ⇧⌘E edit photo, ⇧⌘K crop, ⇧⌘R redact, ⇧⌘B background, ⇧⌘V trim video, ⇧⌘J crop video, ⇧⌘L speed, ⇧⌘S snapshots, ⇧⌘O organize pages), and typed command examples (`500kb`, `frame blur 40`, `collage row 1600 gap 16`, `trim 0:10-0:30`, `pages 3,1,2r`). Add a table "Which tools appear for which format" generated from `tools.json`.

**C7 (S): file actions section.** Six actions: AirDrop, Float shelf, Copy, Move to…, Copy Path, Terminal/iTerm2. Badges per the README ("AirDrop and Terminal not tried in a live session"; the shelf "not tried in a live session").

**C8 (M): screenshots, model-captured.**

Preparation:
```
mkdir -p /tmp/fc-fx && cd /tmp/fc-fx
sips -s format png /System/Library/Desktop\ Pictures/*.heic --out a.png   # or any local image
ffmpeg -f lavfi -i testsrc=duration=8:size=640x360:rate=25 -f lavfi -i sine=d=8 -shortest demo.mp4
# a 3-page demo.pdf via the app's own CombinePdf, or any local PDF
```

Capture (build first with `cargo build`; paths adjusted to the fixtures):
```
FERROCAST_DATA_DIR=/tmp/fc-data FERROCAST_FILES=/tmp/fc-fx/a.png   scripts/shot.sh /tmp/files-wheel-convert.png FERROCAST_VIEW=wheel FERROCAST_WHEEL=convert FERROCAST_WHEEL_SEL=1
... FERROCAST_WHEEL=tools    -> files-wheel-tools.png
... FERROCAST_WHEEL=actions  -> files-wheel-actions.png
FERROCAST_VIEW=convert        -> files-convert-list.png
FERROCAST_VIEW=annotate  -> files-annotate.png      edit -> files-edit.png     crop -> files-crop.png
redact -> files-redact.png     frame -> files-frame.png     collage (two images) -> files-collage.png
organize (pdf) -> files-organize.png
vtrim -> files-vtrim.png   vcrop -> files-vcrop.png   vspeed -> files-vspeed.png   vsnapshot -> files-vsnapshot.png   vredact -> files-vredact.png
shelf -> files-shelf.png
```
- Exact view names come from the dispatch at `draw.rs:108-358`: clipboard, snippets, quicklinks, emoji, settings, notes, vspeed, vcrop, vredact, vtrim, vsnapshot, collage, frame, redact, crop, edit, annotate, organize, welcome, shortcuts, about, processes, calchistory, shelf, convert, wheel, settings-ai, settings-advanced, timers, ai.
- After A6b/A7d/A8b/A9c/A10 land, add `compress`, `meta`, `reorder`, `vsplit`, `atrim`, etc. dispatches in the same unit that adds each window.
- Convert PNG to JPG for the site: `sips -s format jpeg -s formatOptions 82 in.png --out web/public/img/files-x.jpg`, and compare file sizes (the existing images are JPEG).
- Check alpha with `sips -g hasAlpha`; if the shots have a white background, either capture on a dark page container or just rely on the SVG wheel (C4).
- Dark and light variants for the wheel only. Alt text for each.

**C9 (S): SEO and meta.**
- Extend `Base.astro` with optional `canonical`, `ogImage` and `jsonLd` props (defaults keep current output).
- `/files/` title: "Convert & file tools for Mac · Ferrocast". Description of 140-155 characters: "Convert between 275 format pairs and use image, video, PDF and audio tools from a drag-and-drop wheel. Everything runs locally on your Mac."
- JSON-LD: `SoftwareApplication` (`operatingSystem: macOS`, free offer, MIT) and `FAQPage` for the page FAQ.
- Add `/files/` to `web/public/sitemap.xml` (static file).
- One `<h1>`, a heading hierarchy, descriptive link text, a canonical URL under `https://drumst0ck.github.io/ferrocast/files/`, and an og image `og.jpg` reused unless a `files-og.jpg` (1200×630, composed from the wheel and a screenshot) is made.

**C10 (S): copy.** Wording (English), with no third-party branding or assets, and never "Tangerine":
- Hero: "Drag a file. Tap a key. Drop."
- Lead: "Hold a file, tap Shift for conversions, Command for tools, or Control for actions. The wheel stays open until you drop it. Everything runs on your Mac; nothing is uploaded."
- Numbers: "46 input formats, 275 conversion pairs, 26 file tools, 6 file actions", with the footnote "Every conversion pair is exercised in the test suite on generated sample files. Layout fidelity for complex documents is not checked."
- Optional tools line: "ffmpeg and LibreOffice are used when installed; macOS tools come first."
- A "Be honest" box: "Windows were rendered and checked in screenshots; mouse and keyboard behaviour is still being proved in live sessions."

**C11 (S): build verification.** `cd web && npm ci; echo $?`, `npm run build; echo $?`, grep `dist/files/index.html` for "275", verify the `#q=` filter in a headless check if feasible, check that the HTML has no JS-only content, and run a local link check. No publishing, no deploy workflow changes.

**C12 (S, optional):** add a CHANGELOG line "Added: Files page" and refresh `web/public/sitemap.xml`.

### Part C file map
- New: `web/src/pages/files.astro`, `web/src/components/{SiteHeader,SiteFooter,StatusBadge,WheelDemo,MatrixExplorer,ToolCards}.astro`, `web/src/scripts/{wheel-demo,matrix-filter}.js`, `web/src/data/{conversions.json,tools.json,status.ts}`, `web/public/img/files-*.jpg`, `crates/core/examples/site_data.rs`.
- Edited: `web/src/pages/index.astro`, `web/src/layouts/Base.astro`, `web/src/styles/site.css` (new rules scoped under `.files-`), `web/public/sitemap.xml`.

---

## 5. Recommended execution order

```
Phase 0  A1 ───────────────────────────────────────────────┐
Phase 1  B1 → B2 → B5 → B3 → B4                              │
Phase 2  A3, A2, A4 (need A1; A2/A4 need B5)                │
Phase 3  A-T1, A-T2 (event tests; independent, run early)    │
Phase 4  A14 → A15(spike)                                    │
Phase 5  A8a → A8b → A8c;  A9a → A9b → A9c                   │
Phase 6  A6a → A6b → A6c → A6d(spike) → A6e                  │
Phase 7  A7a → A7b → A7c → A7d                               │
Phase 8  A10a → A10b → A10c → A10d/e/f/g                     │
Phase 9  A12a → A11a → A11b → A11c;  A13                     │
Phase 10 A5a → A5b → A5c (opt-in, human-gated)               │
Phase 11 C1 (needs A1) → C2 → C4 → C5 → C6 → C7 → C3 → C9 → C10 → C8 → C11
         (C8 after B6 and after the windows it shows exist)
Phase 12 A-H items as time allows; B6 captures; B7 optional; A16 human
```
Rationale:
- A1 first, because everything keys off the mapping.
- B1/B2/B5 early, so the wheel can show 9 and 12 segments before A2/A4.
- Tests A-T1/A-T2 early, because they de-risk later UI work without a human.
- Smaller, certain gaps before the risky ones (A5, A6d).
- The website needs A1 for tool data, and its screenshots come last.
- Parts B and C are independent of the remaining A units after A1 and B5. Do not interleave unrelated units in one commit.

---

## 6. Instructions for Sonnet

**Guardrails.**
- Read `odd/tasks/file-tools.md` (S10/S11 and the log) first and treat the log as the record. Do not claim "done" without evidence of exit codes.
- Do not add Tangerine branding or assets. Do not name it on the website.
- Do not use `panic=abort`. No heavy per-frame work. Never run processes on the UI thread; use the bus/worker pattern (`Msg::*`, newest-wins).
- Never overwrite an original. Reuse `output_path` / `output_with_suffix`.
- Parsers must use ASCII-lowercasing (L23 regression: slicing with offsets from a Unicode-lowercase copy panicked).
- Enter and Esc must not be consumed while a text field is focused (L41 B1).
- Keep each commit under about 400 changed lines, and split a unit if it grows.
- Never publish the site. Don't edit CI.
- Do not run `codegraph` lifecycle commands other than the sanctioned init and queries.

**Per-unit checklist.**
1. Re-read the unit and the files it touches. Confirm the evidence lines still match (code moves).
2. Write the RED tests, run them, record the failing exit code (compile failure = 101).
3. Implement the minimum change.
4. Run the tests until they pass: `cargo test -p <crate> <name>; echo $?`.
5. Run, as three separate commands without pipes: `cargo fmt --check; echo "fmt=$?"`, `cargo clippy --workspace --all-targets -- -D warnings; echo "clippy=$?"`, `cargo test --workspace; echo "test=$?"`.
6. If UI: take the screenshot with `scripts/shot.sh` and look at it (read the PNG).
7. For web units: `cd web && npm run build; echo $?`.
8. Update README status, CHANGELOG, and the odd log (`L64+`) with exactly what was and was not proven.
9. Commit with the unit id.

**Report back, per unit.**
- The unit id and the commit hash.
- Files changed and the line count.
- The test names added and their RED then GREEN exit codes.
- The three cargo exit codes.
- The screenshot paths, with what you saw.
- Anything not proven, and what a human must check (matching §7 ids).
- Deviations from the plan, and any UNVERIFIED item resolved (with how).

---

## 7. Human real-session checklist

Do these on a real Mac, with Accessibility and Automation granted.

- **H1:** Drag a file in Finder, tap Shift, Command, then Control. The wheel opens under the pointer, switches, stays open on key release, runs the segment on drop, and cancels when released outside the ring.
- **H2:** Arrow keys, Enter and Esc on the wheel; ⌥↵ on a result.
- **H3 (A5):** With "Enter on Finder selection" on: select a file and press Return (the wheel opens). Rename (Return on a selected file with nothing else) still works with the setting off. Return in the Finder search field and in a rename edit is untouched. Quick Look and a Save dialog are unaffected. Typing speed does not degrade, and the tap recovers after sleep.
- **H4:** Float shelf: always on top, pins to an edge, drag in and out. AirDrop picker. Terminal and iTerm opening. Finder selection command (Automation prompt).
- **H5:** Compress, photo edit, background and collage sliders feel responsive, and the estimate is close to the final size on a 12 MP photo.
- **H6:** Crop/redact/annotate mouse dragging, the text field, the callout drag-and-type, and the image-stamp file chooser.
- **H7:** Organizer keys and thumbnails on a large PDF.
- **H8:** Drag-to-reorder in the reorder list, the collage and the PDF organizer.
- **H9:** The audio waveform on a 30-minute file stays responsive; trim, bleep and channels results sound right.
- **H10:** A real RAR archive extracts (needs a real `.rar` fixture created with an arm64 or official tool).
- **H11:** Open a generated `.pages` file in Pages.app.
- **H12:** Native blur (B7) on light and dark wallpapers and with multiple displays, with no flicker.
- **H13:** Wheel visuals in dark and light, for 6, 8 and 9 segments, hover glow, switching animation, Reduce Motion on, and a Retina display.
- **H14:** Rotated phone video through the crop and redact windows, and a variable-frame-rate video through snapshots.
- **H15:** Metadata edits on a real camera JPEG and a real audio file with chapters, checked in another app.
- **H16:** The site: open `npm run dev` or `preview`; check the matrix filter with the keyboard only, mobile widths, reduced motion, and the copy against the README status.