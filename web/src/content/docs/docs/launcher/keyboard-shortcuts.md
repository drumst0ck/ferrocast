---
title: "Keyboard shortcuts"
description: "Every Ferrocast keyboard shortcut in one table: launcher keys, the editor windows for photos, video and PDF, and the keys inside each tool window."
sidebar:
  order: 13
---
This page lists the shortcuts in the launcher and in the file tool windows. Shortcuts are written with macOS glyphs.

## Launcher

| Keys | Action |
|---|---|
| <kbd>⌥</kbd> <kbd>Space</kbd> | Open or close Ferrocast. You can change it in Settings. |
| <kbd>↑</kbd> <kbd>↓</kbd> | Move the selection. |
| <kbd>↵</kbd> | Run the primary action. |
| <kbd>⌘</kbd> <kbd>↵</kbd> | Run the secondary action. |
| <kbd>⌘</kbd> <kbd>K</kbd> | Open the action panel for the selected item. |
| <kbd>⌘</kbd> <kbd>,</kbd> | Open Settings. |
| <kbd>⌘</kbd> <kbd>N</kbd> | New snippet, quicklink or note in those lists. |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>F</kbd> | Add or remove a favorite; on a file result it runs Float on Shelf. |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>,</kbd> | Set a hotkey for a command. |
| <kbd>esc</kbd> | Clear the search, go back, or close. |
| <kbd>⌃</kbd> <kbd>X</kbd> | Delete the selected entry. |
| <kbd>⌘</kbd> <kbd>Y</kbd> | Quick Look the selected file. |
| <kbd>⌘</kbd> <kbd>.</kbd> | Pin or unpin a clipboard entry. |
| <kbd>⌘</kbd> <kbd>E</kbd> | Edit the selected snippet or quicklink. |
| <kbd>⌫</kbd> | Go back when the search is empty. |

## File tools

| Keys | Action |
|---|---|
| <kbd>⌘</kbd> <kbd>T</kbd> | Convert the selected file result. |
| <kbd>⌥</kbd> <kbd>↵</kbd> | Open the format wheel for a file result. |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>A</kbd> | Annotation canvas (image). |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>E</kbd> | Live-preview photo editor (image). |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>K</kbd> | Crop editor (image). |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>R</kbd> | Redaction editor (image). |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>B</kbd> | Background editor (image). |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>V</kbd> | Video trim viewer. |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>J</kbd> | Video crop window. |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>L</kbd> | Video speed window. |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>S</kbd> | Video snapshot viewer. |
| <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>O</kbd> | Visual PDF organizer. |

## Inside the windows

| Window | Keys |
|---|---|
| Any editor | <kbd>↵</kbd> writes the copy and never overwrites; <kbd>esc</kbd> goes back. |
| Drag wheel | Arrow keys choose, <kbd>↵</kbd> applies, <kbd>esc</kbd> cancels; tapping <kbd>⇧</kbd>, <kbd>⌘</kbd> or <kbd>⌃</kbd> switches wheel. |
| Video trim and split | <kbd>←</kbd> <kbd>→</kbd> step a frame, <kbd>⇧</kbd> <kbd>←</kbd> <kbd>→</kbd> step a second, <kbd>I</kbd> <kbd>O</kbd> set markers (trim), <kbd>M</kbd> adds a cut and <kbd>⌫</kbd> removes the last (split), <kbd>Home</kbd> <kbd>End</kbd> jump. |
| PDF organizer | <kbd>R</kbd> rotate, <kbd>⇧</kbd> <kbd>R</kbd> rotate back, <kbd>D</kbd> duplicate, <kbd>⌫</kbd> remove, <kbd>⌘</kbd> <kbd>←</kbd> <kbd>→</kbd> move the page. |
| Choose Order | <kbd>⌘</kbd> <kbd>↑</kbd> <kbd>⌘</kbd> <kbd>↓</kbd> move the row. |

:::caution[Limits]
None of the keys in the file tool windows has been tested with real key presses in a live session; they are covered by synthetic-event tests. <kbd>↵</kbd> on a Finder selection is not included.
:::

## Related

- [Drag wheel](/docs/files/drag-wheel/)
- [Settings](/docs/launcher/settings/)
