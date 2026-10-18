# Changelog

## 0.1.0 — unreleased

First public build.

### Added in the latest iteration
- Kill Process: processes busiest first with app icon, CPU and memory; ↵ terminates, ⌘↵ force-kills.
- Timers: `timer 10m`, `pomodoro`, `set a timer for 1h 30m`; notification and sound when done; Timers view with cancel.
- System commands: Toggle Wi-Fi and Toggle Keep Awake (caffeinate).
- Quick Look for files (⌘Y) and Check for Updates through Homebrew.
- Calculator History (last 100 results, delete one or clear), recently used emoji, and a “Look up in Dictionary” fallback.
- Ask AI providers: besides any OpenAI-compatible server, Sign in with ChatGPT (the OAuth flow and backend OpenAI's Codex CLI uses, so a ChatGPT plan answers without an API key; session in the Keychain, refreshed automatically; the Settings pane states plainly that OpenAI has not blessed third-party use), Anthropic's Messages API (own key in the Keychain) and Claude Code, the official CLI signed in with your Claude subscription (tools off, prompt on stdin, own system prompt). Test connection checks each. ChatGPT and Claude subscriptions do not include API access, so they are not offered directly.
- Welcome, About, Keyboard Shortcuts, Ask AI and the clipboard detail pane redesigned in the same language as Settings: hero icon, tone-stepped cards with rows, key caps grouped by Navigate/Act/Manage, chat bubbles with a provider chip and an empty state with example prompts, and an Information list (type, source, when, times, length, pinned) under every clipboard preview.
- Icons: every builtin and system command uses an SF Symbol rendered by AppKit; glyphs no longer show as empty boxes.
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
