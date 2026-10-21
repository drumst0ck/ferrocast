---
title: "Feature status"
description: "The full Ferrocast status table, one row per feature, using the Tested and Typed labels, with the real limits and requirements of each feature."
sidebar:
  order: 0
---
This is the status table that used to live in the README, using the Tested and Typed vocabulary used across the site. Every feature below is tested; the notes list real limits and requirements.

| Label | Meaning |
|---|---|
| **Tested** | Tested. |
| **Typed** | Typed command only. |
| **Not included** | Not included yet. |

## Notes on permissions and scripts

- Permissions (Accessibility / Input Monitoring) are granted by the user. After recompiling with an ad-hoc signature macOS may ask for them again. Distributing needs Developer ID signing and notarization.
- Window management, global hotkeys, pasting and snippet expansion require Accessibility.
- Scripts with `needsConfirmation` ask for confirmation before running.
- Script Command arguments (including those of type `password`) are passed on the command line, as in Raycast, and can be seen with `ps` while the script runs. Do not put critical secrets there.

## Launcher

| Feature | Status | Notes |
|---|---|---|
| Root Search (apps, commands, frecency ranking, aliases, favorites) | Tested | |
| Calculator (math, %, units), colours, URL detection | Tested | |
| Currency (ECB rates through frankfurter.dev, 12 h cache) and time zones (`time in tokyo`, `5pm ldn in sf`) | Tested | Currency requires a network. |
| ⌘K action panel and shortcuts | Tested | |
| Clipboard history (text, files, images, pin) | Tested | Ignores marks left by password managers. |
| Snippets with placeholders and auto-expansion | Tested | Needs permissions. |
| Quicklinks with arguments | Tested | |
| Raycast-compatible Script Commands | Tested | Modes full, compact, silent, inline. |
| Window management (about 40 commands) | Tested | Needs Accessibility. |
| System commands, emoji, files (Spotlight), window switcher, notes | Tested | |
| Kill Process (processes by CPU and memory, SIGTERM or SIGKILL with ⌘↵) | Tested | |
| Timers (`timer 10m`, `pomodoro`; notification and sound when done; Timers view with cancel) | Tested | |
| Toggle Wi-Fi and Toggle Keep Awake (caffeinate) as system commands | Tested | |
| Quick Look for files (⌘Y) and “Check for Updates” through Homebrew | Tested | |
| Calculator History (last 100 calculations, delete one or all), recent emoji, “Look up in Dictionary” as fallback | Tested | |
| Menu bar icon, settings, per-command hotkeys | Tested | |
| Colour emoji (rendered with AppKit) | Tested | |
| Ask AI with streaming and conversation. Four providers: an OpenAI-compatible server (Ollama by default, LM Studio, OpenAI…), **Sign in with ChatGPT** (same OAuth flow and backend as the Codex CLI; uses your ChatGPT plan; grey area of terms, flagged in the app), the Anthropic API (own key) and **Claude Code** (the official CLI, with your Claude subscription). Keys in the Keychain | Tested | Works with the OpenAI-compatible server, Claude Code and the Anthropic API (your own key). |
| Launch at login (LaunchAgent) | Tested | Applied at the next login. |
| Welcome and permissions, shortcuts, About | Tested | |
| Export backup (without clipboard history), import snippets (Raycast JSON) | Tested | |
| Single instance, error log (`Reveal Log File`) | Tested | |
| Icon, entitlements, DMG, Homebrew formula | Tested | The notarization script is optional. |

## File conversion and tools

| Feature | Status | Notes |
|---|---|---|
| File conversion: all 46 input formats and 275 pairs of the Tangerine table (image, audio, video, documents, spreadsheets, PDF, slides, EPUB, subtitles, archives); macOS tools first, ffmpeg, LibreOffice and cwebp optional | Tested | All 275 pairs run on real generated files and the result is read back (text inside the document, image width, audio or video duration). RAR extraction needs a working `unrar`; `.pages` is written in iWork '09 format. |
| Image tools: compress, resize, crop, remove metadata, combined PDF, rotate, flip, background (colour or blurred copy, padding, corners, shadow: `frame blur 40`), collage (grid, row, column, featured: `collage row 1600 gap 16`), adjustments (`edit brightness 20 grain 30 dehaze 50`), redact faces or text (Vision) or typed areas with a colour or blur (`redact 10,10 200x100 blur`), annotate (own canvas; with several images, Preview) | Typed | Typed commands; the live-preview editors and the canvas are in the rows below (⇧⌘A, ⇧⌘E, ⇧⌘K); another image as background with `frame image /path/bg.jpg`. |
| File actions (copy, copy path, move to…, Terminal/iTerm, AirDrop, compress to ZIP) and File Shelf | Tested | “Zip” is an action added on request (it is not one of Tangerine's six): each selected file or folder gets its own .zip next to it, without overwriting, and the name loses the original extension (`photo.png` becomes `photo.zip`). |
| Drag wheel (Shift: conversions, Control: actions) | Tested | |
| Floating shelf (native always-on-top panel pinned to an edge, drag in and out) | Tested | |
| Tools wheel (Command), compression by size, exact crop, EXIF/GPS metadata, trim/split/merge | Tested | |
| Audio and video tools (ffmpeg): mute, speed, frame, crop, cover with a box or blur, join clips (silence generated for silent ones), normalize, visualizer (waveform or still image), trim silence at the ends, mono/stereo, bleep; organize PDF pages (`pages 3,1,2r`) | Typed | Typed commands. Normalize is single pass and the notice shows the measured loudness before and after (LUFS), with no waveform; WMV fails on mute and crop; no visual editor and no waveform. |
| Format wheel from a file with the keyboard (⌥↵ on a result; Shift/Command/Control switch wheels) | Tested | Enter on a file still opens it. Enter on a Finder selection is an opt-in setting: Settings > General > “Enter on Finder selection” (off by default, needs Accessibility), which opens the Convert wheel instead of renaming. The Convert Finder Selection command also works. |
| Visual PDF organizer (⇧⌘O on a PDF: thumbnails, rotate, duplicate, remove and reorder pages, saves a copy) | Tested | Pages can also be dragged to reorder; a long PDF cannot be dragged past the visible strip, use ⌘←/→. |
| Own annotation canvas (⇧⌘A on an image: arrow, line, box, ellipse, highlight, pencil, numbered labels and magnifier; saves a full-size PNG copy) | Tested | Free text (system Arial font); callouts with a pointer and image overlays; “Annotate” in the tools menu opens the canvas for one image and Preview for several. |
| Photo editor with live preview (⇧⌘E: nine sliders, full-size copy with ↵) | Tested | The preview uses a reduced copy. |
| Crop editor (⇧⌘K: draggable rectangle with handles, fixed ratios, exact width and height; copy with ↵) | Tested | |
| Redaction editor (⇧⌘R: drag to add areas, move and resize them, solid colour or blur; copy with ↵) | Tested | The preview only marks the areas; the cover is applied when saving. |
| Background editor (⇧⌘B: padding, corners and shadow sliders, colour or blurred copy; copy with ↵) | Tested | Colour, blurred copy or another image picked with “Choose image…”. |
| Collage editor (with 2 or more images: grid, row, column or featured; width, spacing, corners and background; copy with ↵) | Tested | Images reorder by dragging their thumbnails. |
| Video trim viewer (⇧⌘V: frame at the cursor, start and end markers, step by frame or second, exact re-encoded cut with ↵; requires ffmpeg) | Tested | Exact re-encoded cut; requires ffmpeg. |
| Video crop window (⇧⌘J: real frame, draggable rectangle, ratios, exact width and height; copy with ↵; requires ffmpeg) | Tested | The reference frame is fixed (10 % of the duration). |
| Video speed window (⇧⌘L: 0.25–4×, presets and resulting duration; requires ffmpeg) | Tested | It does not play the video. |
| Video frame viewer (⇧⌘S: timeline, step by frame or second, ↵ saves a PNG at original resolution) | Tested | |
| Video redaction window with several areas (Redact Video… action): area list, each with its draggable rectangle, time range in seconds and solid black or blur; reference-frame control (also in the crop window); copy with ↵; requires ffmpeg | Tested | The preview only marks the areas. |
| Halo wheel design: accent per wheel (orange, blue, green), highlighted sector with gradient and glow, centre with wheel name, count and kind, every segment named under its icon (from 9 the letters are smaller and long names are cut with “…”; the full name shows in the centre on hover); the actions wheel has Tangerine's six actions plus Zip and the conversion wheel shows every output (those needing an uninstalled tool dimmed, with the install command) | Tested | No animations. |
| Tools of the wheel and of Convert by format (table of 26 tools); those without a window use presets or typed commands | Tested | |
| Extract archives (zip, tar, tar.gz, gz and rar) into a new folder next to the file, after checking there are no absolute paths or `..` | Tested | Symbolic links are resolved by `tar` and `ditto`; `gz` has no size limit. |
| Choose the order when merging PDF, audio or video and when creating a PDF from images (“Choose Order” window) | Tested | ⌘↑/⌘↓ keys and dragging rows. |
| Split video and audio: window with markers (M, Backspace), equal parts, all clips in a new folder | Tested | |
| Compress: images by size, percentage or preset with optional resize; audio and video with ffmpeg (computed bitrate); PDF with optional Ghostscript and presets only | Tested | The size estimate is approximate, not a sample encode; PDF requires Ghostscript. |
| Edit metadata for audio, video, GIF and PDF (window per file, track and chapter; remove all) | Tested | Chapters are read-only; a GIF only supports removal; “remove all” leaves some container tags; images keep typed commands. |
| Trim audio with waveform (“Trim Audio” window: markers, exact start and end boxes, “Trim silence” button), normalize (target in LUFS, range and true peak, with prior measurement), channels (mono or stereo, left and right volume in dB), bleep with several ranges (“Bleep Audio” window: list of ranges over the waveform, each replaced by a 1 kHz tone) and audio to video (“Audio to Video” window: waveform or still image, landscape, portrait or square) | Tested | Bleep and audio to video are offered only for an audio file (the bleep executor rejects video); the silence threshold is per sample (a click shifts it); no before-and-after comparison bar. |
| `/files` web page (interactive wheel, explorer of the 275 conversions with filters, tools with honest status badges) | Tested | Generated from the app data. |
| React extensions, sync, Search Menu Bar Items, AI context (selection/screen) | Not included | |

## Distribution

The supported route is Homebrew compiling from source (see [Install](/docs/getting-started/install/)). The **unnotarized** DMG from `scripts/dmg.sh` will be blocked by Gatekeeper on other Macs that download it; it is for your own use. Notarization (`scripts/notarize.sh`) is optional and requires a paid developer account.

## Related

- [Overview of the file tools](/docs/files/)
- [Frequently asked questions](/docs/reference/faq/)
