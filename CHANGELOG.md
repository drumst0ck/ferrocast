# Changelog

## 0.1.0 — unreleased

First public build.

### Added in the latest iteration
- Kill Process: processes busiest first with app icon, CPU and memory; ↵ terminates, ⌘↵ force-kills.
- Timers: `timer 10m`, `pomodoro`, `set a timer for 1h 30m`; notification and sound when done; Timers view with cancel.
- System commands: Toggle Wi-Fi and Toggle Keep Awake (caffeinate).
- Design: San Francisco as the UI font, brand accent, sliding selection, action-panel pop-in, launcher fade-in.
- Speed: the result list is cached across frames; release builds use fat LTO.

### Fixed
- Calculator: `200 * 10%`, `pi/2`, `2 x 3`, `1,000 * 2`, small currency amounts (`1 jpy in usd`).
- Time zones: `est`/`pst`/`cet` are fixed offsets; clocks inside a DST gap still resolve.
- `{date offset=…}` no longer panics on absurd values; `{snippet name=…}` resolves in pastes and quicklinks.
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
