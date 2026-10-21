## Ferrocast perf baseline — 0d660f5 — Apple M3 — 27.0

| Metric | Result |
|---|---|
| hidden start: window+gl ready, median of 5 | 121.1 ms |
| hidden start: App::new, median | 10.6 ms |
| hidden start: apps scan, median (153 apps on the dev Mac) | 125.8 ms |
| hidden idle (15s after start, 10s window): CPU | 0.55 % |
| hidden idle: context switches/s | 241 |
| hidden idle: frames drawn while hidden (sum of printed 10 s windows: 2; none printed = 0) | 243 |
| hidden idle: slow frames while hidden | 3 |
| physical footprint after 15s hidden / after idle window | 159.1M / 163.4M |
| view: root (empty query) | sections max n/a ms; icon sync 8 ms (3 renders); slow frames 2 |
| view: root query 'saf' | sections max n/a ms; icon sync 6 ms (3 renders); slow frames 2 |
| view: clipboard, 1000 text + 50 images | sections max 2.31775 ms; icon sync 9 ms (4 renders); slow frames 3 |
| view: clipboard, query 'line 5' | sections max 1.41529 ms; icon sync 6 ms (4 renders); slow frames 3 |
| view: emoji, empty | sections max 0.931625 ms; icon sync 43 ms (12 renders); slow frames 7 |
| view: emoji, query 'smile' | sections max 0.488042 ms; icon sync 23 ms (9 renders); slow frames 6 |
| view: settings | sections max n/a ms; icon sync 5 ms (3 renders); slow frames 3 |

## After P2 (hidden launcher no longer drawn), 3 runs, same Mac

Cause found with the new `hidden repaint cause` lines: the whole UI was still laid out and painted while
hidden, so the focused search box blinked its caret (egui `text_selection/visuals.rs:313`, ~every 0.5 s),
pending icons re-requested frames (`ui.rs:105`) and each frame flushed an invisible GL surface.

| Metric | Before (P0 baseline) | After P2 |
|---|---|---|
| frames drawn while hidden, 10 s after start | 105 per 10 s window | 7-8 (startup only) |
| frames drawn while hidden afterwards | 243 in 20 s | 0 (3 runs, 4 windows) |
| idle hidden CPU | 0.55 % | 0.00 % |
| idle hidden context switches/s | 241 | 42-44 (the 25 ms wheel poller, P8) |
| physical footprint 15 s after hidden start | 159 MB | 85-90 MB |

## After P1 (apps scan cache), same Mac

| Metric | Before | After P1 |
|---|---|---|
| `apps scan` at start, no cache yet | 126 ms | 130 ms (parses everything once, then writes `apps-cache.plist`, 25 KB) |
| `apps scan` at start, cache present | 126 ms | 7.1 ms (median of 5 second starts) |
| `list_apps` real scan, release test | 141 ms | 5.5 ms warm; result identical to the uncached scan (ignored test `real_scan_is_identical_with_and_without_the_cache`) |

## P4 dropped, P7 and the texture part of P5 done

- **P4 (Arc sections, `show_rows`) dropped on evidence**: steady frame cost measured with
  `list_frame_timing_clipboard_1000_and_emoji` (release, after warm-up): Clipboard with 1000 entries
  184 µs, Emoji 67 µs, Root 14 µs per frame. egui already culls off-screen rows, so the per-frame clone
  and layout are far below the 16 ms budget. The visible hitches seen in the screenshot probe are
  first-frame costs (16-60 ms) shared by every view.
- **P7**: `IconBytes` no longer marks the list dirty (up to 40 rebuilds per shown list, ~0.5 ms each).
- **P5 textures**: a clipboard picture was read and decoded at full size on the UI thread and its
  texture stayed for the whole session. Measured with a 2880x1800 screenshot-sized PNG: 52 ms on the UI
  thread and a 20 MB texture per picture, unbounded. Now a worker downscales to 720 px (1.3 MB texture),
  the UI shows a placeholder meanwhile, and at most 32 clipboard textures are kept (LRU), so the worst
  case is ~50 MB instead of growing with every picture viewed. egui also panics on textures above the
  GPU limit, which a very large screenshot could hit.
