# Changelog

## 0.1.0 — unreleased

First public build.

### Added in the latest iteration
- File tools: drop files on the launcher, press ⌘T on a file result or run Convert Files to convert images, audio, video, documents, spreadsheets, subtitles and folders. Built-in macOS tools come first (sips, afconvert, avconvert, textutil, ditto); ffmpeg, LibreOffice and cwebp are optional, and a missing one shows its install command. Output goes next to the original and never overwrites it.
- Image tools: compress, resize, crop to an aspect ratio, remove metadata (lossless for JPEG and PNG), combine into one PDF, rotate, flip, square on a white or black background, collage, one-step edits (black & white, brighter, darker, more contrast), redact faces or text on-device with Vision, and annotate a copy in Preview.
- File actions on file results: Copy File (paste it in Finder), Copy Path, Move to… (never overwrites), Open in iTerm or Terminal, AirDrop, Float on Shelf.
- Drop wheel: while dragging files, tap Shift for a ring of conversions or Control for file actions; release over a segment to run it. Keys and on/off in Settings → General. No Accessibility permission needed.
- Tools wheel (Command) with arrow-key, Enter and Escape control; tapping Shift, Command or Control switches an open wheel.
- Typed image tools: `collage row 1600 gap 16 radius 12 bg 000000` (grid, row, column, featured), `frame blur 40 pad 80 shadow 30` (colour or blurred background, padding, corners, shadow), `edit brightness 20 contrast 10 saturation -30 warmth 15 clarity 40 sharpen 30 grain 20 denoise 30 dehaze 50`, `redact faces`, `redact text blur 25`, `redact 10,10 200x100 300,40 80x80 color ff0000`. There is no live preview or canvas; `frame image /path/bg.jpg` uses another picture as the background.
- Typed audio, video and PDF tools: `mute`, `speed 1.5x`, `frame 1:23`, `crop 640x360 at 100,50`, `redact 10,10 200x100 0:05-0:09 blur`, `normalize -14`, `mono`, `stereo`, `silence`, `bleep 0:05-0:07`, `visualizer square image /path/cover.jpg`, `pages 3,1,2r`, and Join for several clips (silent clips get generated silence). Normalize is single pass and the result toast shows the measured loudness before and after (for example `-31.0 → -16.0 LUFS`, measured with ffmpeg's ebur128); there is no waveform or compare view, and WMV fails on mute and crop.
- Format wheel from a file by keyboard: ⌥↵ on a file result opens the conversion wheel; tapping Shift, Command or Control on an open wheel switches it. Not tested with real key presses; ↵ on a Finder selection is not included.
- Compress to a size or percentage (`500kb`, `40%`), exact crop (`800x600`), camera and GPS tags (`make Canon`, `gps 40.4,-3.7`), trim/split/merge audio, video and PDF (`trim 0:10-0:30`, `split 30s`).
- All 275 conversions of Tangerine's table across its 46 input formats, including PDF to XLSX, EPUB and Pages, documents to Markdown, EPUB and Pages, XLS input, text clippings, GIF to video and archive repacking. The real-file test runs every pair and reads the result back (text inside documents and sheets, image width, media duration). Limits: layout fidelity is not checked; RAR is tested only with a hand-built archive because the bundled `unrar` is x86-only; `.pages` is written in the iWork '09 format and was opened with LibreOffice, not with Pages.
- More formats: WMA, WMV, EPUB, PPTX from PDF, ODS, TSV, TAR, SVG input, PDF to images (every page, 300 DPI) and to text, subtitles to text, slides to images.
- Floating shelf: a native always-on-top panel pinned to a screen edge; drop files in, drag them out. Not yet verified in a real session.
- File Shelf: keep files at hand in a persisted list with every file action; drop files on the open shelf to add them.
- Kill Process: processes busiest first with app icon, CPU and memory; ↵ terminates, ⌘↵ force-kills.
- Timers: `timer 10m`, `pomodoro`, `set a timer for 1h 30m`; notification and sound when done; Timers view with cancel.
- System commands: Toggle Wi-Fi and Toggle Keep Awake (caffeinate).
- Quick Look for files (⌘Y) and Check for Updates through Homebrew.
- Calculator History (last 100 results, delete one or clear), recently used emoji, and a “Look up in Dictionary” fallback.
- Ask AI providers: besides any OpenAI-compatible server, Sign in with ChatGPT (the OAuth flow and backend OpenAI's Codex CLI uses, so a ChatGPT plan answers without an API key; session in the Keychain, refreshed automatically; the Settings pane states plainly that OpenAI has not blessed third-party use), Anthropic's Messages API (own key in the Keychain) and Claude Code, the official CLI signed in with your Claude subscription (tools off, prompt on stdin, own system prompt). Test connection checks each. ChatGPT and Claude subscriptions do not include API access, so they are not offered directly.
- Welcome, About, Keyboard Shortcuts, Ask AI and the clipboard detail pane redesigned in the same language as Settings: hero icon, tone-stepped cards with rows, key caps grouped by Navigate/Act/Manage, chat bubbles with a provider chip and an empty state with example prompts, and an Information list (type, source, when, times, length, pinned) under every clipboard preview.
- Icons: every builtin and system command uses an SF Symbol rendered by AppKit; glyphs no longer show as empty boxes.
- Ask AI: the provider chip is a menu to switch between the providers that are ready right now (configured server, ChatGPT signed in, Anthropic key saved, Claude Code installed) and between that provider's models, without leaving the chat. The choice is saved and the warm Claude session restarts when needed.
- Settings → AI: the model is a dropdown fed by each provider (the server's /models list for OpenAI-compatible and Anthropic, the ChatGPT backend catalogue, Claude Code's aliases), with Default and Custom… entries and a refresh button.
- Ask AI with Claude Code keeps one warm CLI session while the view is open, started in the background when you open Ask AI: measured 3.2 s for a cold first answer versus 1.4 s for every following turn, with the conversation context kept by the CLI.
- Settings redesigned: a sidebar with General, Clipboard, Snippets, AI, Scripts and Advanced; each setting is a row with a title, a plain-language description and its control (animated switches, segmented pickers, key caps). The AI tab gains provider presets (Ollama, LM Studio, OpenAI), Keychain status for the key and a Test connection button that asks the server for its model list.
- Footer shows the app icon and a pulsing “Searching files” indicator while Spotlight answers.
- Design: San Francisco as the UI font, brand accent, sliding selection, action-panel pop-in, launcher fade-in.
- Speed: icon decoding on the main thread is capped at 4 ms per frame (an emoji takes 5–30 ms to render), app icons are fetched on a background thread before the list asks for them, and rows scrolled out of view are no longer painted or fetched (the emoji list went from 80 icon loads to 7). Hotkey to drawn frame measures about 2 ms, a keystroke to new results 0.1–0.3 ms. The result list is cached across frames; release builds use fat LTO; the 22 MB Unicode fallback font loads only when non-Latin text appears (about 40 MB less memory at idle, 146 MB in one process on an M3).

### Fixed
- Calculator: `200 * 10%`, `pi/2`, `2 x 3`, `1,000 * 2`, small currency amounts (`1 jpy in usd`).
- Time zones: `est`/`pst`/`cet` are fixed offsets; clocks inside a DST gap still resolve.
- `{date offset=…}` no longer panics on absurd values; `{snippet name=…}` resolves in pastes and quicklinks.
- A single click runs a list row (hover already selects); clicks in the 350 ms after a view change are ignored so a habitual double click cannot fire the first row of the new screen.
- Snippet keywords that prefix another keyword are rejected; held Backspace no longer closes the launcher.
- Confirm view no longer stays under a script's output; ⌘↵ never fires "Add to Favorites" by position.
- Deleted clipboard images are removed from disk; currency rates refresh when stale; failed saves are reported.
- A malformed `file://` URL on the pasteboard cannot panic the clipboard watcher.

### Initial feature set

- Root Search with fuzzy matching, frecency, aliases, favorites and per-command hotkeys; ⌘K action panel.
- Calculator: math, percentages, units, currencies (ECB rates) and time zones; colour conversion.
- Clipboard history (text, files, images, pinning; password-manager entries are skipped).
- Snippets with dynamic placeholders and automatic expansion; quicklinks with arguments.
- Script Commands compatible with Raycast metadata (full, compact, silent, inline output).
- Window management (about 40 layouts), system commands, emoji (colour), file search, window switcher, notes.
- Ask AI with streaming and conversation against any OpenAI-compatible server; key stored in the Keychain.
- Welcome and permissions screen, keyboard shortcut help, backup export, snippet import, launch at login.
- Single-instance lock and an error log; app icon, entitlements, DMG and notarization scripts.
