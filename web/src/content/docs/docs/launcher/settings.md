---
title: "Settings and backup"
description: "Ferrocast Settings has ten tabs, from General and Clipboard to Capture, Integrations and Advanced. Set hotkeys, login launch and backups."
sidebar:
  order: 12
---
Settings is a sidebar with ten tabs: General, Clipboard, Snippets, AI, Translate, Capture, Notes, Scripts, Integrations and Advanced. <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>⌘</kbd> <kbd>9</kbd> and <kbd>⌘</kbd> <kbd>0</kbd> jump to them in that order. Each setting is a row with a title, a plain-language description and its control (animated switches, segmented pickers, key caps).

Open it with <kbd>⌘</kbd> <kbd>,</kbd>. There is also a menu bar icon.

## What you can set

- The launcher hotkey (default <kbd>⌥</kbd> <kbd>Space</kbd>) and per-command hotkeys.
- Launch at login through a LaunchAgent; it applies at the next login.
- The drag-wheel keys and on/off in Settings → General (see [Drag wheel](/docs/files/drag-wheel/)).
- Updates (Settings → General): check automatically, ask first or install automatically, and Check now (see [Install](/docs/getting-started/install/#updates)).
- Every translation option lives in Settings → Translate: the translate key (Command by default), the languages, what happens after translating, and the engine that translates: an AI provider and model, or Apple Translation, which works offline on macOS 26 (see [Translate selected text](/docs/launcher/translate/)).
- Settings → Capture has the screenshot folder (empty uses `~/Pictures/Ferrocast Captures`), the format (PNG only), **After capture** (Show, Copy or Save), **Include cursor**, the **Recording frame rate** (30 or 60 fps) and **Recording countdown** (Off, 3 s or 5 s), and one shortcut row each for Capture Area, Capture Window, Capture Screen, Capture Scrolling, Record Screen, Record Area and Record Window, all empty by default (see [Screen capture](/docs/launcher/screen-capture/)).
- Settings → Notes has three options for the floating Notes window: a menu bar button that shows or hides it, whether Esc moves focus away instead of closing it, and the zoom (see [Notes](/docs/launcher/notes/)).
- The AI provider, model and keys (see [Ask AI](/docs/launcher/ask-ai/)).
- Backup: export your data (clipboard history is not included) and import snippets from a Raycast JSON export.

:::caution[Limits]
Launch at login is applied at the next login. Failed saves are reported.
:::

## Related

- [Notes](/docs/launcher/notes/)
- [Keyboard shortcuts](/docs/launcher/keyboard-shortcuts/)
- [Data and privacy](/docs/reference/data-and-privacy/)
