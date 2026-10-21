---
title: "File actions"
description: "Seven Ferrocast file actions for any file or folder: Copy, Copy Path, Move to, Terminal or iTerm, AirDrop, Float on Shelf and Zip. Never overwrites."
sidebar:
  order: 6
---
File actions work on any file or folder, including mixed selections and files that have no conversion tools. There are seven: **AirDrop**, **Float** (keep files on a floating shelf), **Copy**, **Move to…**, **Copy Path**, **Terminal / iTerm2** and **Zip**.

Hold <kbd>⌃</kbd> while dragging files to open the Actions wheel, or use the actions in <kbd>⌘</kbd> <kbd>K</kbd> on a file result (Zip appears there as Compress to ZIP).

| Action | What it does |
|---|---|
| AirDrop | Send the selected files with the system sharing picker. |
| Float shelf | Keep files on a small floating shelf that stays on top and pins to a screen edge. Files can be dropped onto it. |
| Copy | Put the files on the clipboard, ready to paste in Finder. |
| Move to… | Pick a destination folder and move the files there. An existing file is never overwritten. |
| Copy Path | Copy the path of each selected item, one per line. |
| Terminal / iTerm2 | Open a folder, or the folder that holds a file, in Terminal or iTerm2. |
| Zip | Compress each selected file or folder into its own ZIP next to it. An existing ZIP is never overwritten. |

:::caution[Limits]
Zip is an action added on request: it is not one of the six actions of the app this project is measured against. Each selected file or folder gets its own `.zip` next to it, without overwriting, and the name loses the original extension (`photo.png` becomes `photo.zip`). Terminal for several files opens only the first file's location.
:::

## Related

- [Shelf](/docs/files/shelf/)
- [Drag wheel](/docs/files/drag-wheel/)
