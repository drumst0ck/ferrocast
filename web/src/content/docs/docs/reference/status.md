---
title: "Feature status"
description: "The full Ferrocast status table, one row per feature, using the Tested, Built and Typed labels, with every verification note and limit preserved."
sidebar:
  order: 0
---
This is the status table that used to live in the README, translated to the Built, Tested and Typed vocabulary used across the site. Every note is preserved; nothing was upgraded.

| Label | Meaning |
|---|---|
| **Tested** | Run on real files in tests. |
| **Built** | Implemented, not tried in a live session. |
| **Typed** | Typed command only. |
| **Not included** | Not included yet. |

The original wording “Implemented” (Implementado) maps to **Built** unless the row says it was run on real files, which maps to **Tested** (conversions, the Tools wheel typed tools) or **Typed** (typed commands).

## What is verified and what is not

- Verified: tests (core, bindings, UI state), starting the `.app`, and the real look of the window through internal screenshots (`scripts/shot.sh`): search, calculator, currency, time zones, clipboard with detail, the ⌘K panel, emoji and settings.
- **Not verified in a real session**: focus when opening and closing, global hotkeys, pasting, snippet expansion and window management (they require Accessibility, which the user grants).
- Permissions (Accessibility / Input Monitoring) are granted by the user. After recompiling with an ad-hoc signature macOS may ask for them again. Distributing needs Developer ID signing and notarization.
- Scripts with `needsConfirmation` ask for confirmation before running.
- Script Command arguments (including those of type `password`) are passed on the command line, as in Raycast, and can be seen with `ps` while the script runs. Do not put critical secrets there.

## Launcher

| Feature | Status | Notes |
|---|---|---|
| Root Search (apps, commands, frecency ranking, aliases, favorites) | Built | |
| Calculator (math, %, units), colours, URL detection | Built | |
| Currency (ECB rates through frankfurter.dev, 12 h cache) and time zones (`time in tokyo`, `5pm ldn in sf`) | Built | Currency requires a network. |
| ⌘K action panel and shortcuts | Built | |
| Clipboard history (text, files, images, pin) | Built | Ignores marks left by password managers. |
| Snippets with placeholders and auto-expansion | Built | Needs permissions. |
| Quicklinks with arguments | Built | |
| Raycast-compatible Script Commands | Built | Modes full, compact, silent, inline. |
| Window management (about 40 commands) | Built | Needs Accessibility. |
| System commands, emoji, files (Spotlight), window switcher, notes | Built | |
| Kill Process (processes by CPU and memory, SIGTERM or SIGKILL with ⌘↵) | Built | |
| Timers (`timer 10m`, `pomodoro`; notification and sound when done; Timers view with cancel) | Built | |
| Toggle Wi-Fi and Toggle Keep Awake (caffeinate) as system commands | Built | Not tested in a real session. |
| Quick Look for files (⌘Y) and “Check for Updates” through Homebrew | Built | Not tested in a real session. |
| Calculator History (last 100 calculations, delete one or all), recent emoji, “Look up in Dictionary” as fallback | Built | |
| Menu bar icon, settings, per-command hotkeys | Built | |
| Colour emoji (rendered with AppKit) | Built | |
| Ask AI with streaming and conversation. Four providers: an OpenAI-compatible server (Ollama by default, LM Studio, OpenAI…), **Sign in with ChatGPT** (same OAuth flow and backend as the Codex CLI; uses your ChatGPT plan; grey area of terms, flagged in the app), the Anthropic API (own key) and **Claude Code** (the official CLI, with your Claude subscription). Keys in the Keychain | Built | OpenAI tested against a simulated server, Claude Code tested for real (`cargo test claude_code_live -- --ignored`), Anthropic API not tested for real. |
| Launch at login (LaunchAgent) | Built | Applied at the next login. |
| Welcome and permissions, shortcuts, About | Built | |
| Export backup (without clipboard history), import snippets (Raycast JSON) | Built | |
| Single instance, error log (`Reveal Log File`) | Built | |
| Icon, entitlements, DMG, Homebrew formula | Built | The notarization script is optional and has not been run. |

## File conversion and tools

| Feature | Status | Notes |
|---|---|---|
| File conversion: all 46 input formats and 275 pairs of the Tangerine table (image, audio, video, documents, spreadsheets, PDF, slides, EPUB, subtitles, archives); macOS tools first, ffmpeg, LibreOffice and cwebp optional | Tested | All 275 pairs run on real generated files and the result is read back (text inside the document, image width, audio or video duration). Layout fidelity is not proven; RAR only with a synthetic file, because the `unrar` on this Mac is x86; `.pages` is written in iWork '09 format and opened only with LibreOffice, not with Pages. |
| Image tools: compress, resize, crop, remove metadata, combined PDF, rotate, flip, background (colour or blurred copy, padding, corners, shadow: `frame blur 40`), collage (grid, row, column, featured: `collage row 1600 gap 16`), adjustments (`edit brightness 20 grain 30 dehaze 50`), redact faces or text (Vision) or typed areas with a colour or blur (`redact 10,10 200x100 blur`), annotate (own canvas; with several images, Preview) | Typed | Implemented as typed commands, tested with real files; the live-preview editors and the canvas are in the rows below (⇧⌘A, ⇧⌘E, ⇧⌘K); another image as background with `frame image /path/bg.jpg`; faces: no test with a real photo of a face. |
| File actions (copy, copy path, move to…, Terminal/iTerm, AirDrop, compress to ZIP) and File Shelf | Built | “Zip” is an action added on request (it is not one of Tangerine's six): each selected file or folder gets its own .zip next to it, without overwriting, and the name loses the original extension (`photo.png` becomes `photo.zip`); AirDrop, Terminal and Zip not tested in a real session. |
| Drag wheel (Shift: conversions, Control: actions) | Built | Drawing verified by screenshot; the real gesture is untested. |
| Floating shelf (native always-on-top panel pinned to an edge, drag in and out) | Built | Not tested in a real session. |
| Tools wheel (Command), compression by size, exact crop, EXIF/GPS metadata, trim/split/merge | Tested | Tested with real files. |
| Audio and video tools (ffmpeg): mute, speed, frame, crop, cover with a box or blur, join clips (silence generated for silent ones), normalize, visualizer (waveform or still image), trim silence at the ends, mono/stereo, bleep; organize PDF pages (`pages 3,1,2r`) | Typed | Implemented as typed commands; tested with real files and reviewed by an independent verifier with no blockers. Normalize is single pass and the notice shows the measured loudness before and after (LUFS), with no waveform; WMV fails on mute and crop; no visual editor and no waveform. |
| Format wheel from a file with the keyboard (⌥↵ on a result; Shift/Command/Control switch wheels) | Built | No test with real keys in a session. Enter on a file still opens it; Enter on the Finder selection is not included. |
| Visual PDF organizer (⇧⌘O on a PDF: thumbnails, rotate, duplicate, remove and reorder pages, saves a copy) | Built | Drawing verified by screenshot, keys untested in a real session; dragging thumbnails to reorder tested only with synthetic events (the real gesture and automatic scrolling with long PDFs untested; the PDF written after a drag was not tested, only the plan). |
| Own annotation canvas (⇧⌘A on an image: arrow, line, box, ellipse, highlight, pencil, numbered labels and magnifier; saves a full-size PNG copy) | Built | Drawing verified by screenshot and saving with a real file; mouse and keys untested in a real session; free text (system Arial font); callouts with a pointer and image overlays (file chooser, untested in a real session); “Annotate” in the tools menu opens the canvas for one image and Preview for several. |
| Photo editor with live preview (⇧⌘E: nine sliders, full-size copy with ↵) | Built | Drawing verified by screenshot, sliders untested in a real session; the preview uses a reduced copy. |
| Crop editor (⇧⌘K: draggable rectangle with handles, fixed ratios, exact width and height; copy with ↵) | Built | Drawing verified by screenshot and crop with a real file; dragging untested in a real session. |
| Redaction editor (⇧⌘R: drag to add areas, move and resize them, solid colour or blur; copy with ↵) | Built | Drawing verified by screenshot, dragging untested in a real session; the preview only marks the areas. |
| Background editor (⇧⌘B: padding, corners and shadow sliders, colour or blurred copy; copy with ↵) | Built | Drawing verified by screenshot, sliders untested in a real session; colour, blurred copy or another image picked with “Choose image…”; the native file dialog is untested in a real session. |
| Collage editor (with 2 or more images: grid, row, column or featured; width, spacing, corners and background; copy with ↵) | Built | Drawing verified by screenshot, sliders untested in a real session; images reorder by dragging their thumbnails (synthetic events only, the real gesture untested). |
| Video trim viewer (⇧⌘V: frame at the cursor, start and end markers, step by frame or second, exact re-encoded cut with ↵; requires ffmpeg) | Built | Drawing verified by screenshot with a real frame, keys and bar untested in a real session; preview latency not measured. |
| Video crop window (⇧⌘J: real frame, draggable rectangle, ratios, exact width and height; copy with ↵; requires ffmpeg) | Built | Drawing verified by screenshot, dragging untested in a real session; the reference frame is fixed (10 % of the duration). |
| Video speed window (⇧⌘L: 0.25–4×, presets and resulting duration; requires ffmpeg) | Built | Screenshot and rendering tests verified; slider interaction untested in a real session; it does not play the video. |
| Video frame viewer (⇧⌘S: timeline, step by frame or second, ↵ saves a PNG at original resolution) | Built | Event tests and screenshot verified; real-session interaction and exactness with variable-frame-rate video untested. |
| Video redaction window with several areas (Redact Video… action): area list, each with its draggable rectangle, time range in seconds and solid black or blur; reference-frame control (also in the crop window); copy with ↵; requires ffmpeg | Built | Executor tested with real video (pixels, overlapping areas, rotated video) and the window verified by screenshot and synthetic events; dragging and keys untested in a real session; the preview only marks the areas; the reference frame has no real run against ffmpeg and no measured latency. |
| Halo wheel design: accent per wheel (orange, blue, green), highlighted sector with gradient and glow, centre with wheel name, count and kind, every segment named under its icon (from 9 the letters are smaller and long names are cut with “…”; the full name shows in the centre on hover); the actions wheel has Tangerine's six actions plus Zip and the conversion wheel shows every output (those needing an uninstalled tool dimmed, with the install command) | Built | Drawing verified by screenshot in dark and light; no animations (not done), real gesture, real transparency and Retina untested. |
| Tools of the wheel and of Convert by format (table of 26 tools); those without a window use presets or typed commands | Built | Tested with synthetic events (arrows, Esc, switching wheels), not with real keys. |
| Extract archives (zip, tar, tar.gz, gz and rar) into a new folder next to the file, after checking there are no absolute paths or `..` | Built | Reviewed by an independent verifier; RAR only tested with a synthetic archive read with `tar`; symbolic links are resolved by `tar` and `ditto`; `gz` has no size limit. |
| Choose the order when merging PDF, audio or video and when creating a PDF from images (“Choose Order” window) | Built | ⌘↑/⌘↓ keys and dragging rows (dragging tested only with synthetic events); Enter running the merge untested. |
| Split video and audio: window with markers (M, Backspace), equal parts, all clips in a new folder | Built | Executor tested with real video and audio; window not tested with real mouse and keyboard; WMV, variable-frame-rate video and cuts without re-encoding untested. |
| Compress: images by size, percentage or preset with optional resize; audio and video with ffmpeg (computed bitrate); PDF with optional Ghostscript and presets only | Built | The size estimate is approximate, not a sample encode; PDF requires Ghostscript (no proven native route); quality and large files untested. |
| Edit metadata for audio, video, GIF and PDF (window per file, track and chapter; remove all) | Built | Tested with real files; chapters are read-only; a GIF only supports removal; “remove all” leaves some container tags; images keep typed commands; window not tested with real mouse and keyboard. |
| Trim audio with waveform (“Trim Audio” window: markers, exact start and end boxes, “Trim silence” button), normalize (target in LUFS, range and true peak, with prior measurement), channels (mono or stereo, left and right volume in dB), bleep with several ranges (“Bleep Audio” window: list of ranges over the waveform, each replaced by a 1 kHz tone) and audio to video (“Audio to Video” window: waveform or still image, landscape, portrait or square) | Built | Sample extractor, normalization, bleep (measured with Goertzel) and audio to video tested with real audio; windows not tested with real mouse and keyboard; bleep and audio to video are offered only for an audio file (the bleep executor rejects video); the native image dialog of audio to video is untested; the silence threshold is per sample (a click shifts it); no before-and-after comparison bar. |
| `/files` web page (interactive wheel, explorer of the 275 conversions with filters, tools with honest status badges) | Built | Generated from the app data and checked in a local browser; not published; no screenshots of the new windows. |
| React extensions, sync, Search Menu Bar Items, AI context (selection/screen) | Not included | |

## Distribution

The supported route is Homebrew compiling from source (see [Install](/docs/getting-started/install/)). The **unnotarized** DMG from `scripts/dmg.sh` will be blocked by Gatekeeper on other Macs that download it; it is for your own use. Notarization (`scripts/notarize.sh`) is optional and requires a paid developer account.

## Related

- [Overview of the file tools](/docs/files/)
- [Frequently asked questions](/docs/reference/faq/)
