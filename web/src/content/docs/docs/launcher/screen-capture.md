---
title: "Screen capture"
description: "Take screenshots and record your screen from the launcher: area, window, whole screen or a scrolling page. Needs macOS 14 and Screen Recording."
sidebar:
  order: 15
---
Ferrocast takes screenshots from the launcher or from a hotkey you choose. Each capture is saved as a PNG and opens in a small Capture window where you can copy it, annotate it, copy the text in it or pin it above your other windows.

It also records your screen. A recording is saved as a project folder with the video and a log of what you did with the mouse. Editing and export are coming later: today you can record, browse and reveal your recordings in Finder.

## The four screenshot commands

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

These apply to both screenshots and recordings.

- **macOS 14 or later.** On macOS 13 the commands are hidden. If you have a shortcut set, pressing it shows, once per session: `Screen capture needs macOS 14. This Mac runs macOS 13.` If Ferrocast was built without its capture helper, the message is `Screen capture is not available in this build.`
- **Screen Recording permission.** Without it you get this message and System Settings opens; no half-written file is left:

  `Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.`

Capture Scrolling also needs Accessibility, because Ferrocast sends the scroll events. If it is off, the message is `Scrolling capture needs Accessibility. Turn Ferrocast on in System Settings → Privacy & Security → Accessibility, then try again.`

Recording also needs Screen Recording. Without it you get the same message, System Settings opens and the half-made project folder is removed.

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

## Screen recording

### The commands

| Command | What it does |
|---|---|
| **Record Screen** | Records a whole display. |
| **Record Area** | Choose an area, as in Capture Area, then record it. |
| **Record Window** | Choose a window, as in Capture Window, then record it. |
| **Open Last Recording** | Shows your newest recording in Finder. See the note on the editor below. |
| **Recordings** | Lists your recordings, newest first. |

Each one has the subtitle “Capture” and no default shortcut. Set shortcuts in Settings → Capture → Shortcuts.

### Starting and stopping

After you choose the target, a countdown runs (3 s by default; Settings → Capture → **Recording countdown** offers Off, 3 s and 5 s). Running the command again during the countdown cancels it. Then a small recording panel appears with the elapsed time, **Pause** (it becomes **Resume**) and **Stop**. The panel is not in the video.

While recording, the menu bar icon shows `●  mm:ss` and the tray menu starts with **Stop Recording**. Stop from the panel or from that menu item. While one recording runs, you cannot start another. A toast tells you the file is saved, with its name and length.

The frame rate is 60 fps by default; Settings → Capture → **Recording frame rate** also offers 30 fps. The recording is as long as the wall-clock time between the end of the countdown and Stop, even if the screen does not change.

### What is saved

Each recording is a folder named `Recording YYYY-MM-DD at HH.MM.SS.ferrocast` in `~/Movies/Ferrocast Recordings`. To use another folder, set `capture.recordings_folder` in `settings.json`. There is no Settings row for it yet. Inside:

| File | What it holds |
|---|---|
| `screen.mov` | The video. The mouse pointer is not drawn into it: the editor will draw it later from `events.json`. |
| `events.json` | A log of the cursor position (about 60 times a second), clicks, scrolling, and the position of the windows. For keys it stores only shortcuts (a key pressed with a modifier other than <kbd>⇧</kbd>) so they can be shown. Plain typing is never stored beyond the time of each key press, so a recording cannot contain a password. |
| `project.json` | The recording's settings and its duration. |
| `thumb.png` | A still picture of the recording, used in the Recordings list. |

Everything stays on your Mac.

### Input Monitoring

Input Monitoring is needed only to log shortcut keys. Without it, recording still works and you see this message when it starts: `Keyboard shortcuts will not be shown: Input Monitoring is off.` Clicks, scrolling and the cursor do not need it.

### Low disk space

Below 2 GB free, a warning shows `Low disk space: N GB left`. Below 200 MB, the recording stops by itself, cleanly, and keeps the file. The toast says `Recording stopped: low disk space.` followed by the project name and length.

### Recordings and Open Last Recording

**Recordings** lists the projects in the recordings folder, newest first, with their length and size.

| Key | Action |
|---|---|
| <kbd>↵</kbd> | Show in Finder |
| <kbd>⌘</kbd> <kbd>⌫</kbd> | Move to Trash (asks to confirm) |
| <kbd>Esc</kbd> | Go back |

The editor is not available yet. **Open Last Recording** therefore only reveals the newest project in Finder and says `Showing NAME in Finder. The editor is not available yet.` If there is no recording, it says `No recordings yet`. Editing and export are coming later; there is no date.

:::caution[Limits]
- PNG only. Settings → Capture shows the format as PNG, with no other choice. **Include cursor** draws the pointer in area, window and screen captures.
- The scrolling capture keeps all its strips in memory until it finishes.
- Recordings cannot be edited or exported yet, and there is no camera or audio recording. Open them from Finder.
- A recording is lost if you quit Ferrocast while it runs. Stop it first.
:::

## Related

- [Settings](/docs/launcher/settings/)
