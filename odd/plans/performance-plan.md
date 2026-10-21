# Ferrocast performance plan

Date: 2026-10-21 · Author: analysis session (Claude Fable 5.1, read-only) · Executor: a later session (Sonnet)
Baseline build: `target/release/ferrocast` from 81545f9 (HEAD c6cb3e7 only adds docs), Apple M3, macOS 26.
Nothing in this document was implemented. Every number below was measured, not estimated; every suspect without a number says so.

## How the baseline was measured

```bash
# hidden start with a private data dir, timings from FERROCAST_PROFILE
D=$(mktemp -d); FERROCAST_DATA_DIR=$D FERROCAST_PROFILE=1 ./target/release/ferrocast --hidden 2> $D/prof.log &
sleep 15; top -l 11 -s 1 -pid $PID -stats pid,cpu,mem,idlew,csw      # 10 s of idle
sample $PID 10 1 -file $D/sample.txt                                   # where the main thread is
vmmap --summary $PID                                                   # memory regions
# one view/query, exits after a few frames, prints "sections" and "icon sync" timings
FERROCAST_DATA_DIR=$D FERROCAST_PROFILE=1 FERROCAST_SCREENSHOT=$D/x.png FERROCAST_VIEW=clipboard ./target/release/ferrocast
```

## Baseline numbers

| Metric | Measured | Notes |
|---|---|---|
| `window+gl ready` | 105–215 ms warm, 528 ms cold | eframe/glow window creation; app normally starts at login, so low priority |
| `App::new` | ~10 ms warm, 64–105 ms cold | fonts 21 ms, hotkeys+tray 74 ms (cold only) |
| `apps scan` (`list_apps`, 153 apps) | 120 ms warm, 200–390 ms cold | **known regression**: was 12 ms before 8f74f2c (localized names parse every `InfoPlist.loctable`, 83 files, 2.1 MB). Runs on a thread at start and again on every `show()` after 10 min (`app.rs:1248-1251`) |
| `show -> frame` | 1–5 ms | fine |
| `sections` Root, 153 apps | 0.5 ms first, 10–20 µs cached | fine |
| `sections` Clipboard, 1000 text entries | ~2 ms, 1.2 ms with a query | acceptable; the per-frame draw of 1000 rows is NOT measured (no frame-time probe exists) |
| Idle hidden, fresh `--hidden` start (15–25 s after launch) | 0.4–1.2 % CPU, 200–380 context switches/s, RSS 148–159 MB swinging ±10 MB every second | `sample` shows the main thread inside `-[NSOpenGLContext flushBuffer]` 474 ms out of 10 s: **frames are rendered while the window is hidden** |
| Idle hidden, the user's instance running 24 min (shown and hidden before) | 0.0 % CPU, ~50 context switches/s, no `flushBuffer` in a 5 s sample, 54 MB RSS / 97 MB top MEM | the 50 switches/s are the 25 ms wheel poller (40/s) + the 400 ms clipboard poller |
| Memory regions at start (vmmap) | IOSurface 27.3 MB + IOAccelerator 18.8 MB (GL window buffers, present while hidden), Malloc Small 47.8 MB, Malloc Large 15 MB (fonts: SFNS.ttf 7.9 MB, Apple Symbols 0.9 MB; Arial Unicode 23 MB when the wide font is loaded) | |
| Sync icon rendering on the UI thread (`icon sync`) | SF symbols 1–12 ms each; emoji 5–30 ms each (code comment, `ui.rs:81`) under a 4 ms frame budget | emoji view (300 items) pops in over many frames |

Not measured: frame time of any view, the emoji view fill time, the cost of using the file tools (previews, big video), clipboard image texture memory. P0 adds the probes for these.

## What already looks fine (do not re-investigate)

- No continuous repaint while visible: only conditional `request_repaint_after` (`draw.rs:33,77,81`, `app.rs:1288`, `tick_inline` every 10 s only if an inline script exists).
- App icons are prefetched off-thread at 64 px and cached as textures (`services.rs:143-153`, `ui.rs:83-88`).
- The sections list is cached per query/view (`app.rs:2329`); search does not spawn a thread per keystroke; `mdfind` is debounced 200 ms (`draw.rs:62-78`).
- Clipboard poller only reads `changeCount` every 400 ms; images are separate PNG files, not base64 in JSON.
- Release profile: `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`, `strip = true`.
- Conversion/file tools run on demand in separate processes (measured in `odd/tasks/file-tools.md` L127: +1.6 MB binary, +2 MB RSS, no idle cost).

## Plan (ordered; do P0 first, then by priority)

Each task: evidence → change → how to prove → acceptance → risk. Keep each task its own work-unit commit with the before/after numbers in the message. Run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace` per commit.

### P0 — Measurement harness (prerequisite)

Evidence: there is no frame-time or hidden-frame probe; `sample` was the only way to see hidden rendering.
Change:
1. Under `FERROCAST_PROFILE`, print per-frame `ui()` duration when it exceeds 4 ms, and a counter of frames drawn while `!self.visible` printed once per 10 s (`draw.rs` `logic`/`ui`).
2. `scripts/perf.sh`: reproduces the table above (hidden start timings, 10 s `top`, `sample` grep for `flushBuffer`, `vmmap --summary` physical footprint) and the view shots (`FERROCAST_VIEW=clipboard|emoji`, with a generated 1000-entry `clipboard.json` fixture and 50 synthetic images in `clipboard-images`). Output one table so before/after is a diff.
3. Pure timing tests (ignored by default, `--ignored`) for `ranking::score` over 500 items and `App::sections` for Clipboard with 1000 entries, printing µs.
Acceptance: `scripts/perf.sh` runs end to end and prints the baseline numbers above within noise.
Risk: none (profiling code behind the env var; no behavior change).

### P1 — `list_apps` regression: 104 ms → ~12 ms warm

Evidence: `odd/tasks/file-tools.md` L127; `crates/macos/src/apps.rs:50-69,130-155` parse the full `InfoPlist.loctable` per app and `read_dir` Resources per language; two attempts (hoisting languages, 8 threads) measured no gain, so the cost is I/O/parse bound.
Change: on-disk cache in the data dir (`apps-cache.json`): key = app path + `Info.plist` mtime + `.loctable` mtime + preferred languages; value = name, bundle id, localized names. On scan, read the cache, reparse only entries whose key changed, write the cache when anything changed. Keep `list_apps()` signature; `rebuild()` untouched.
Prove: `FERROCAST_PROFILE` `apps scan` line, warm, 7 runs median, before/after; the existing localized-name test (`captura de pantalla`) still passes; add a test that a stale mtime invalidates one entry.
Acceptance: warm scan ≤ 25 ms with a warm cache; cold (no cache) unchanged; no missing or stale app after an app update (mtime changes).
Risk: item 1 light (a cache file is written in the data dir; corrupt cache must be ignored, never fatal).

### P2 — Stop rendering while hidden

Evidence: fresh hidden instance draws frames (474 ms/10 s in `flushBuffer`, 0.5–1 % CPU, 200–380 csw/s, ±10 MB RSS churn); the long-lived user instance does not, so a transient requester exists after start (hypotheses, unproven: egui `TextEdit` cursor blink while the search box keeps focus, icon prefetch messages each calling `request_repaint` in `Bus::send` `services.rs:117`, `rates` refresh).
Change: first measure with P0 (hidden frame counter) in three states: `--hidden` start, after show→hide, 5 min later. Then identify the requester with `ctx.repaint_causes()` printed under `FERROCAST_PROFILE`. Likely fixes: surrender egui focus and clear `show_t` on `hide()`; skip `request_repaint` in `Bus::send` for `IconBytes` when `!visible` and drain them on the next show; make `logic` return early for the hidden window except `drain`/`sweep_timers`.
Prove: P0 counter = 0 frames over 60 s hidden in all three states; `top` csw ≤ 60/s; `sample` shows no `flushBuffer`.
Acceptance: as above, and show→frame latency unchanged (≤ 5 ms), toasts and timers still repaint when visible.
Risk: item 6 only if the hidden counter is not tested; add a test that `hide()` leaves no pending repaint request (egui `ctx.has_requested_repaint()`).

### P3 — Emoji view: render glyphs off the UI thread and persist them

Evidence: `ui.rs:104-118` renders emoji via AppKit on the UI thread (5–30 ms each) with a 4 ms/frame budget, so a 300-item list fills over many frames with a repaint each (`ui.rs:106`); emoji search filters all `emojis::iter()` with `contains` and sorts per keystroke (`app.rs:2466-2471`).
Change: route `Icon::Emoji` (and `Icon::Glyph` SF symbols) through the same prefetch path as app icons (`services::prefetch_app_icons`), rendering on a thread at the needed size; persist the PNG bytes in `images_dir/emoji/<codepoints>.png` so the next launch skips AppKit; keep the 4 ms budget only for texture upload. Precompute a lowercase search index for emoji once (name + shortcodes) instead of per keystroke.
Prove: P0 frame-time probe while opening the emoji view and typing `smile`; time from open to last icon drawn (count `icon sync` lines → should be 0 after warm cache).
Acceptance: no `ui()` frame > 4 ms while filling the emoji list; second launch shows the full list within 2 frames; search per keystroke ≤ 1 ms for the 300-cap.
Risk: item 1 light (new cache files in the data dir; never required, regenerate on miss).

### P4 — Per-frame clones and non-virtualized lists

Evidence: `sections_cached` clones the whole `Vec<Section>` every frame and builds a `format!` key (`app.rs:2330-2335`); `draw.rs:1431` clones the selected item; the main list draws every row inside a plain `ScrollArea::show` (`draw.rs:1664-1680`), so Clipboard (uncapped, up to 1000 unpinned + pinned) and Emoji (300) lay out every row each frame.
Change: store the cache as `Arc<Vec<Section>>` (or `Rc`) and return a clone of the handle; compare the key fields instead of formatting a string; use `ScrollArea::show_rows` with the fixed row height for the flat list (sections have headers: compute a flat index of (section, row) once per cache fill and feed `show_rows`), keeping keyboard selection scroll-to behavior.
Prove: P0 frame-time probe on the Clipboard view with the 1000-entry fixture and on Emoji, before/after; existing render tests pass unchanged (several tests read ctx data hooks for positions, keep them).
Acceptance: Clipboard 1000 entries steady frame ≤ 2 ms; no visual change in screenshots (`scripts/shot.sh` diff for root, clipboard, emoji, both themes).
Risk: item 3 light (shared list drawing is used by every list view; pin with the existing screenshot/render tests plus one for scroll-to-selected after `show_rows`).

### P5 — Clipboard view: filtering without cloning, bounded image textures

Evidence: `app.rs:2373-2379` clones the entire history under the mutex and does `format!` + `to_lowercase()` per entry per keystroke (2 ms at 1000 entries, measured; grows with 200 KB text entries, `services.rs:191`); `ui.rs:111-120` loads clipboard images at native size with `fs::read` + `decode_png` on the UI thread; `IconCache.map` is insert-only (no `free_texture` anywhere), so every selected clipboard image stays resident as RGBA forever.
Change: filter under the lock with a hoisted lowercase query and `str::contains` on title and content separately (no `format!`), building items only for matches; load `ClipImage` through the prefetch thread and downscale to the detail-pane size (≈ 2× the pane in points, like `services::display_png`); add an LRU cap (e.g. 32 clip textures) with `ctx.tex_manager().write().free()` / dropping the `TextureHandle` on eviction; drop clip textures when leaving the Clipboard view.
Prove: P0 fixture with 50 images: RSS after selecting all 50 before/after (vmmap physical footprint); `sections` µs at 1000 entries before/after; frame-time probe when selecting a 4000 px image (must not exceed 4 ms on the UI thread).
Acceptance: RSS growth bounded (≤ 32 textures × downscaled size); no UI frame > 4 ms when selecting an image; search results identical (test with the fixture: same ids for a few queries).
Risk: none beyond item 3 light (shared `IconCache` used by every icon kind; app icons must keep their behavior — pin with a test that app icon keys are never evicted).

### P6 — Clipboard recording: save and prune outside the lock

Evidence: `record()` holds the `clipboard` mutex while pretty-serializing the whole list (`services.rs:274`, `storage.rs:55`) and running `prune_images` (a `read_dir` + O(entries×files) scan, `storage.rs:102-110`) on every copy; the UI thread takes the same lock in `sections()` (`app.rs:2373`), so a copy during typing can stall a frame; `clipboard::read()` re-encodes TIFF/PNG to PNG and hashes it per image copy (`clipboard.rs:107-146`), on the monitor thread (fine).
Change: mutate the list under the lock, clone only what must be saved (or serialize to bytes under the lock, write after releasing it); run `prune_images` only when an entry was dropped by retention or an image was added; keep pretty JSON (the README promises readable JSON).
Prove: a test that `record` releases the lock before the file write (inject a slow `Store` or assert ordering with a probe); P0 frame-time probe while a script copies 20 items/s.
Acceptance: no UI frame > 4 ms during rapid copies; `clipboard.json` content identical before/after for the same sequence.
Risk: item 1 light (the saved file must stay atomic: tmp + rename as today).

### P7 — Invalidate sections only on messages that change the list

Evidence: `app.rs:1553` sets `sec_dirty = true` for every bus message, including each of up to 40 `IconBytes` per list (`services.rs:143-153`), so the list is rebuilt several times after each query.
Change: set `sec_dirty` per message kind (Apps, Files, Wake, scripts, timers…), not for `IconBytes`/`FrameLoaded`-style payloads.
Prove: count `sections` prints under `FERROCAST_PROFILE` after typing `saf` (before: many; after: 1–2).
Acceptance: results unchanged (existing tests); `sections` rebuilds per keystroke ≤ 2.
Risk: item 6 unless pinned: add a test that an `IconBytes` message does not mark sections dirty and that `Apps` does.

### P8 — Wheel poller wake-ups (optional, low value)

Evidence: `services.rs:277-314` wakes every 25 ms forever when the wheel is on (40 wakeups/s, ~0 % CPU, the bulk of the 50 csw/s measured on the idle instance).
Change: poll at 100 ms while no mouse button is down and switch to 25 ms once `left_down()` is true; keep `Tracker` semantics (tests in `core::wheel` unchanged).
Prove: `top` csw/s idle before/after (≈ 50 → ≈ 15); the wheel still opens with a Shift tap within 50 ms of the key (synthetic sample test).
Acceptance: csw/s idle ≤ 20; `core::wheel` tests pass.
Risk: none (drag detection latency bounded by the slow interval only until the button is seen down; document the 100 ms worst case).

### P9 — Memory at start (investigation, decide after numbers)

Evidence: 150 MB RSS at a hidden start vs 54 MB after 24 min; 46 MB of GL surfaces present while hidden; fonts: 7.9 MB SFNS + 0.9 MB symbols (+23 MB Arial Unicode when `wide_font` triggers, `app.rs:823-826`), each held as `Arc<FontData>` plus egui's parsed copies.
Change: measure the physical footprint by region at start, after show/hide, after the emoji view, after 10 clip images (`vmmap --summary`, `heap`); then decide: (a) nothing if steady-state stays ≤ 100 MB, (b) drop the Arial Unicode load in favor of a smaller CJK font or lazy subset, (c) experiment branch with the `wgpu` eframe backend (native Metal instead of the OpenGL-over-Metal translation shown in `sample`) measuring RSS, `window+gl ready`, binary size and idle CPU; adopt only on a clear measured win.
Acceptance: a written table in this file with the numbers and a decision; no code change unless (b)/(c) win.
Risk: (c) is item 5 (dependency/renderer change): keep it on a branch, separate commit, needs the user's decision.

### P10 — Cold start of `hotkeys`+tray (optional)

Evidence: `hotkeys 74 ms` cold, ~0 warm (`app.rs:919-946`, includes `make_tray`); app starts at login hidden, so user impact is nil.
Change: only if P9 shows it matters, move `make_tray` after the first frame. Otherwise skip.

## Out of scope

- File conversion and media tools (on demand, separate processes; already measured in L127).
- Search ranking: 0.5 ms per keystroke for 153 apps; `fuzzy_match` allocates two `Vec<char>` per call (`fuzzy.rs:52-64`) and `to_lowercase` twice per item (`ranking.rs:108-109`), but it is not on any measured critical path. Revisit only if P0's ranking timing test exceeds 2 ms with 500 items.
- Startup window creation time (eframe internals).

## Execution notes for the implementing session

- Work on a branch (`perf/…`), one commit per task with before/after numbers from `scripts/perf.sh` in the body.
- Never regress `scripts/shot.sh` screenshots; several render tests read ctx data hooks for click positions — keep them.
- Measure first (P0), then change; a task whose measured gain is below noise is dropped, not merged.
- Close each task with a `Risk:` line and update `odd/tasks/` (create `odd/tasks/performance.md` from this plan's S/T structure at the first write).
