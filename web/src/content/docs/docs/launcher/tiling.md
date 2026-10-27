---
title: "Automatic window tiling"
description: "Optional, always-on tiling in Ferrocast: every new window gets a place, four layouts, focus mode, a layout per display, and it pauses with Stage Manager."
sidebar:
  order: 9
---
Automatic tiling gives every new window a place on its display, so you do not drag or resize anything. It is **off by default** because it moves your windows. Turn it on in Settings → Advanced → Window tiling, or run **Tile Windows On/Off** from the launcher.

## Layouts

Pick one in Settings, from the launcher (**Tiling Layout 1–4**) or with <kbd>⌃</kbd> <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>4</kbd>. The shortcuts change the layout of the display that holds the focused window; each display keeps its own layout.

1. **Columns**: windows side by side.
2. **Main + stack**: the first window on the left, the others stacked on the right.
3. **Grid**: a square-ish grid, filled row by row.
4. **Spiral**: each window takes half of what is left.

**Space** (default 8 pt) is the margin to the screen edge and the gap between windows.

## Focus mode

<kbd>⌃</kbd> <kbd>⌥</kbd> <kbd>Z</kbd> makes the focused window fill its display. Press it again and every window returns to exactly where it was.

## Moving windows

<kbd>⌃</kbd> <kbd>⌥</kbd> with an arrow key swaps the focused window with its neighbour in that direction. Past the edge of a display it goes to the next display.

## What is left alone

- Dialogs, sheets, panels, floating and system windows, minimised and full-screen windows are never tiled.
- Settings → Advanced → **Apps that are not tiled** keeps an app out of the layout.
- When an app refuses a size, Ferrocast learns the smallest size it accepts, moves the split lines to make room and floats the window in the middle when it cannot fit. The list is stored on your Mac in the data folder, by app, and never contains window titles.
- When the screen is full, the extra windows stack in the last slot as an overlapping cascade.
- While **Stage Manager** is on, tiling pauses; it resumes when you turn Stage Manager off.
- **Pause or Resume Tiling** in the menu bar menu stops it without moving any window.

## Other options

- **Activate windows on hover** raises the window under the pointer after it rests there.
- **Highlight border** and **Border color** are stored; the border itself is not drawn yet.

:::caution[Limits]
Needs the Accessibility permission. Ferrocast checks windows about every half second, so a new window takes a moment to find its place.
:::

## Related

- [Window management commands](/docs/launcher/window-management/)
