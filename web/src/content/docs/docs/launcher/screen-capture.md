---
title: "Screen capture"
description: "Capture an area, a window, the whole screen or a scrolling page as a PNG. Copy, save, annotate, add a background, copy text or pin it. Needs macOS 14."
sidebar:
  order: 15
---
Ferrocast takes screenshots from the launcher or from a hotkey you choose. Each capture is saved as a PNG and opens in a small Capture window where you can copy it, annotate it, copy the text in it or pin it above your other windows.

Screen recording and a video editor are not available yet. Only screenshots work today.

## The four commands

Type any of these in the launcher. Each one has the subtitle “Capture”.

| Command | What it does |
|---|---|
| **Capture Area** | Dims every display and shows a crosshair and the size in pixels (`W × H`). Drag to choose the area. <kbd>Space</kbd> switches to window mode, <kbd>Esc</kbd> cancels without saving anything. |
| **Capture Window** | The same overlay, already in window mode: click the window you want. |
| **Capture Screen** | Captures the whole display. |
| **Capture Scrolling** | Choose an area, then Ferrocast scrolls it and stitches the pieces into one tall image. See below. |

None of them has a default shortcut. Set one in Settings → Capture → Shortcuts.

Captures are saved at the display's native pixel size (twice the points on a Retina screen) as `Capture YYYY-MM-DD at HH.MM.SS.png`. If that name exists, ` 2` is added. The default folder is `~/Pictures/Ferrocast Captures`.

## Requirements

- **macOS 14 or later.** On macOS 13 the commands are hidden. If you have a shortcut set, pressing it shows, once per session: `Screen capture needs macOS 14. This Mac runs macOS 13.` If Ferrocast was built without its capture helper, the message is `Screen capture is not available in this build.`
- **Screen Recording permission.** Without it you get this message and System Settings opens; no half-written file is left:

  `Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.`

Capture Scrolling also needs Accessibility, because Ferrocast sends the scroll events. If it is off, the message is `Scrolling capture needs Accessibility. Turn Ferrocast on in System Settings → Privacy & Security → Accessibility, then try again.`

## The Capture window

After a capture, Settings → Capture → **After capture** decides what happens: **Show** opens the Capture window, **Copy** only copies the image, **Save** only saves the file. The window has six buttons. Every one of them works on the saved file.

| Button | Shortcut |
|---|---|
| **Copy** (copies the image and closes) | <kbd>↵</kbd> |
| **Save** | <kbd>⌘</kbd> <kbd>S</kbd> |
| **Annotate…** (opens the annotation editor) | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>A</kbd> |
| **Background…** (opens the add-background editor) | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>B</kbd> |
| **Copy Text** | <kbd>⌘</kbd> <kbd>T</kbd> |
| **Pin** | <kbd>⌘</kbd> <kbd>P</kbd> |

## Copy Text

Copy Text reads the text in the image with the macOS Vision framework, on your Mac, and puts it on the clipboard in reading order. A toast says `Copied N characters`, or `No text found`. The image is not sent anywhere.

## Pin

Pin opens the image in a borderless window that stays above your other windows, at its captured size in points. Drag it by its background. Zoom from 25 % to 400 % with the scroll wheel, a pinch, <kbd>+</kbd> and <kbd>-</kbd>; <kbd>0</kbd> goes back to the captured size. <kbd>Esc</kbd> closes it. You can have several pins at once.

## Scrolling capture

Choose the area that scrolls, and Ferrocast scrolls it for you, takes a strip after each step and joins the strips into one image.

- It stops when the content stops moving (two identical strips) or after 60 screens, with the toast `Scrolling capture stopped at 60 screens`.
- If it loses track of the overlap, it saves the screens captured before that point and says so.
- <kbd>Esc</kbd> cancels it. Clicking does not.
- The mouse pointer is never drawn in the strips.

:::caution[Limits]
- PNG only. Settings → Capture shows the format as PNG, with no other choice. **Include cursor** draws the pointer in area, window and screen captures.
- The scrolling capture keeps all its strips in memory until it finishes.
- There is no screen recording and no video editor yet. The Recording rows in Settings → Capture are placeholders for a later version.
:::

## Related

- [Settings](/docs/launcher/settings/)
