---
title: "Screen capture"
description: "Take screenshots, record your screen, then edit it with auto zoom and export MP4 or GIF, all from the launcher. Needs macOS 14 and Screen Recording."
sidebar:
  order: 15
---
Ferrocast takes screenshots from the launcher or from a hotkey you choose. Each capture is saved as a PNG and opens in a small Capture window where you can copy it, annotate it, copy the text in it or pin it above your other windows.

It also records your screen. A recording is saved as a project folder with the video and a log of what you did with the mouse. You can then edit it in Ferrocast, with automatic zooms on your clicks, and export it as an MP4 or a GIF.

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
| **Open Last Recording** | Opens your newest recording in the editor. |
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
| `screen.mov` | The video. The mouse pointer is not drawn into it: the editor draws it from `events.json`. |
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
| <kbd>↵</kbd> | Open in the editor |
| <kbd>⌘</kbd> <kbd>↵</kbd> | Show in Finder |
| <kbd>⌘</kbd> <kbd>⌫</kbd> | Move to Trash (asks to confirm) |
| <kbd>Esc</kbd> | Go back |

**Open Last Recording** opens the newest recording in the editor. If there is none, it says `No recordings yet`.

### Camera, microphone and system audio

Settings → Capture → **Sources** has **Camera**, **Microphone** and **System audio** switches (all off by default) and a device list for the camera and the microphone (**Default** unless you pick one; **Refresh** reloads the list). When a switch is on, Record Screen, Area and Window also write `camera.mov`, `mic.m4a` and `system.m4a` next to `screen.mov`, each with its own start offset so they stay in sync.

The first time a source is used, macOS asks for Camera or Microphone permission when you start the recording. If it is denied, the recording does not start and a notice opens the matching System Settings pane. System audio uses the Screen Recording permission. If a camera or microphone disappears while recording, the recording continues and a notice says which source was lost.

In the editor, a **Camera** tab appears when the recording has a camera: show or hide it, a **Layout** (**Overlay** or **Split**), **Circle**, **Rounded** or **Square**, one of the four corners, size (10 to 40 %), margin and mirror, plus a **Look**, **Cutout (beta)** and **Follow face** (see [Recording effects](#recording-effects)). The camera stays fixed in the frame while the screen zooms. An **Audio** tab appears when there is a microphone or system track: gain per track (-60 to +12 dB), mute per track and a master gain. MP4 exports mix the tracks into one AAC audio track (48 kHz stereo) that follows your trims and speed changes; a speed change also changes the pitch. GIFs have no audio.

## Editing a recording

The editor shows the preview on the left, on a neutral surface, and a panel with tabs on the right (Zoom, Loupe, Cursor, Style and Clips, plus Camera, Audio and Transcript when the recording has them). Under the preview there is a bar with **Play**/**Pause**, **Undo**, **Redo**, the time, **Export…** and **Full screen**, and under it the timeline: a ruler and three named tracks, **Video** (trims and speed changes), **Zooms** and **Loupes**, with the playhead. The time reads `output time / output length`, with the source time next to it when there is room, so you see the length after your edits. At the right of the title bar you see the number of zooms (and of loupes, when there are any) and the save state. <kbd>Esc</kbd> closes the editor and writes anything not yet saved.

### Full screen

**Full screen** (at the right of the bar) or <kbd>⌃</kbd> <kbd>⌘</kbd> <kbd>F</kbd> makes the editor fill the screen, below the menu bar and beside the Dock. The preview grows, the panel widens to about 320–350 pt, and the timeline tracks get taller. Press the same button (it now reads **Exit full screen**) or the same keys to go back to the size and place the window had. <kbd>Esc</kbd>, going back, opening another view or hiding the launcher also bring the window back to its normal size. This is not macOS native full screen (no new Space): the launcher window is a floating panel, so the editor stretches it over the visible part of the screen instead.

The cursor you see is drawn by the editor from the recorded events in `events.json`. It is not in `screen.mov`, which is why you can resize it, smooth it or remove it.

### Playback and keys

| Key | Action |
|---|---|
| <kbd>Space</kbd> | Play or pause |
| <kbd>←</kbd> / <kbd>→</kbd> | One frame back or forward |
| <kbd>⇧</kbd> <kbd>←</kbd> / <kbd>⇧</kbd> <kbd>→</kbd> | One second back or forward |
| <kbd>Home</kbd> / <kbd>End</kbd> | Jump to the start or the end |
| <kbd>⌘</kbd> <kbd>Z</kbd> / <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>Z</kbd> | Undo / Redo |
| <kbd>⌘</kbd> <kbd>E</kbd> | Export… |
| <kbd>⌃</kbd> <kbd>⌘</kbd> <kbd>F</kbd> | Full screen / Exit full screen |

Click or drag on the timeline to move the playhead. While you type in a number field of the panel, only <kbd>⌘</kbd> <kbd>E</kbd> and <kbd>⌃</kbd> <kbd>⌘</kbd> <kbd>F</kbd> keep working. The one-frame step uses the frame rate of Settings → Capture → **Recording frame rate**.

### Auto zoom

Auto zoom zooms toward your clicks. Clicks that are close in time and place form one group. A zoom starts 0.4 s before the first click of a group and returns to the whole screen after 1.2 s without clicks or cursor movement. The default level is 2×.

When you open a recording that has no zooms, Ferrocast works them out and applies them in memory only. Nothing is written to `project.json` until you change something. Exporting a recording you have not touched saves its `project.json` first, with those zooms.

Auto zoom is on by default. `capture.auto_zoom` and `capture.zoom_scale` in `settings.json` turn it off or set the level (1× to 6×); there is no Settings row for them yet. The **Auto zoom** button of the Zoom tab uses the same level.

### The panel

Every change can be undone and redone (<kbd>⌘</kbd> <kbd>Z</kbd>, <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>Z</kbd>, or the **Undo** and **Redo** buttons). Changes of the same kind made within one second, such as dragging a slider, are one step. A change that would make the project invalid is refused and the reason shows in red above the controls.

**Zoom**

- **Add at playhead** adds a manual zoom of 2 s, shorter if the next zoom is close. If the playhead is inside a zoom you get `There is already a zoom at the playhead.`
- **Auto zoom** replaces the automatic zooms with new ones. Manual zooms stay and win any overlap.
- Click a zoom in the list to edit it: **Start**, **End**, **Scale** (1× to 6×), **Focus** (X and Y, in %), **Ease in** and **Ease out** (0 to 3 s). Editing a zoom makes it manual, so Auto zoom keeps it.
- **Pick on preview** sets the focus where you click the preview. **Follow cursor** makes the zoom follow the pointer again. **Delete zoom** removes it.

**Cursor**

- **Default**, **None** and **Hidden**: the pointer, or no pointer (None and Hidden do the same).
- **Size** (0.5× to 4×), **Smoothing** (0 to 100 %), **Click ring** (a ring where you clicked) and **Motion blur** (0 to 200 %).

**Style**

- **Aspect**: 16:9, 4:3, 1:1, 9:16 or Source.
- **Background**: **Colour**, **Gradient** or **Blur** (the recording itself, blurred), with colour pickers and an **Angle** for gradients.
- Eight gradient presets: Aurora, Dusk, Ocean, Graphite, Sand, Lilac, Forest and Rose. A preset is stored as a gradient, not as a wallpaper image.
- **Padding** (0 to 45 %), **Corners** (0 to 64 px) and the shadow: **Shadow** (blur, 0 to 120 px), **Opacity** and **Offset** (0 to 80 px).

**Clips**

- **Trim in** cuts everything before the playhead and **Trim out** everything after it. **Split** splits the segment at the playhead. **Delete segment** removes the segment at the playhead.
- **Speed**: pick 0.25×, 0.5×, 1×, 2×, 4×, 8× or 16× (or type a value), then **Apply to segment**.
- Below, the list of segments (click one to move the playhead) and the length before and after your edits.

### Transcript

The **Transcript** tab appears when the recording has a microphone or system audio track, or already has a transcript. It turns the speech into text you can read and cut, and everything runs on your Mac: the audio and the text never leave it.

The first time, the tab shows a **Language** list (**System language** by default) and a **Transcribe** button. The first time you press **Transcribe**, macOS asks for the **Speech Recognition** permission (the same prompt may also ask for Microphone access). If it was denied, the tab says `Speech Recognition permission is needed. Open System Settings > Privacy & Security > Speech Recognition and allow Ferrocast.` and offers a button that opens that pane. The languages that work are the ones whose on-device models your Mac has; if one is not available, the tab says so. A bar shows the progress and **Cancel** stops it and leaves nothing behind. The text is saved with the project (`transcript.json` next to `project.json`), so it is not worked out again.

Then the tab shows the words as flowing text. The word under the playhead is highlighted. Click a word to move the playhead to it (a word that is already cut moves the playhead to the nearest kept moment). Shift-click or drag to select a range of words. The text is read-only: you cut words, you do not retype them. Cut words are struck through and dimmed, and filler words are marked in orange.

- **Delete selection** cuts exactly the selected words.
- **Remove fillers** cuts filler words such as `um` and `uh` (and `like` or `you know` only when they are isolated or unclear). English and Spanish have a list; other languages have none.
- **Tighten pauses** shortens silences longer than 0.7 s to about 0.25 s.

Each action cuts the timeline like **Clips** does, is one step for <kbd>⌘</kbd> <kbd>Z</kbd>, and tells you how many cuts and seconds it removed, or that there was nothing to remove. The cuts are saved and exported like any other edit.

:::caution
Accuracy depends on the audio quality, so check the words before you cut. macOS 14 and later use the Speech framework. The newer macOS 26 engine is used only when its language assets are already installed; Ferrocast never downloads them.
:::

### Recording effects

These effects are made in the editor and drawn over the recording when you play it and when you export it. They never change `screen.mov` or `camera.mov`, so you can turn each one off again. Each is one step for <kbd>⌘</kbd> <kbd>Z</kbd> and saved with the project. They apply to the preview and to MP4 and GIF exports, and they make an export a little slower.

#### Loupe

A loupe is a magnifying glass over the screen: a round lens that shows part of the screen larger for a few seconds. It magnifies the recording itself, so the text inside stays sharp, and it moves with the zoom. Use it to point at a small detail without zooming the whole screen.

The **Loupe** tab has **Add at playhead**, which adds a loupe of 3 s (shorter if the next loupe is close; if the playhead is inside a loupe you get `There is already a loupe at the playhead.`). Click a loupe in the list to edit it:

- **Start** and **End**. Loupes cannot overlap.
- **Scale** (1.2× to 4×) and **Radius** (5 to 30 % of the width of the picture).
- **Focus**: **Follow cursor** keeps the lens on the pointer; **Pick on preview** sets a fixed spot where you click the preview.
- **Ease in** and **Ease out** (0 to 3 s): how long the lens takes to fade in and out.
- **Delete loupe** removes it.

The timeline has a row of loupe segments under the zoom row. Click a segment to select it and open its settings; the selected one is outlined.

#### Split screen

By default the camera is a bubble over the screen. In the **Camera** tab, **Layout** → **Split** puts the screen and the camera side by side instead, so the camera never covers anything.

- **Side**: Left, Right, Top or Bottom, the side the camera takes.
- **Ratio** (25 to 50 %): how much of the picture the camera takes along that direction.
- **Gap** (0 to 10 % of the width): the space between the two parts.

The screen is shown whole in the rest of the picture, and the camera fills its part (so its edges can be cropped). **Mirror** still applies. **Shape**, **Corner**, **Size** and **Margin** are for the bubble and are hidden in a split; they come back when you choose **Overlay**.

#### Looks

A look changes the colour of the camera picture only. The screen is not touched. In the **Camera** tab, **Look** offers **None** and six looks: **Warm**, **Cool**, **Cinematic**, **Faded**, **High contrast** and **Mono**. **Intensity** (0 to 100 %) blends the look with the original picture.

To use your own colour table, put a `.cube` file (a 3D LUT) inside the recording's project folder and point to it in `project.json`. There is no file picker. Close the recording in the editor first, because the editor rewrites `project.json` when you change something. Inside the `camera` object, add:

```json
"look": { "file": "my-grade.cube", "intensity": 0.8 }
```

The name must be a plain file name that ends in `.cube`, in the project folder itself (no folders). `intensity` is optional and defaults to 1. The Look picker then shows `Custom look: my-grade.cube` with no chip selected; choosing a chip replaces it. If the file is missing or is not a valid `.cube`, the preview and the export say so instead of ignoring it.

#### Cutout (beta)

**Cutout (beta)** removes the background behind you. It uses the macOS Vision framework to find the person, on your Mac. In the bubble layout the person is drawn without the bubble shape and without a shadow, and the whole camera picture is kept inside the camera area, resting on its bottom edge, so a head near the side of the frame is not cut off. It is a beta: the quality of the edge depends on the light and on the background (a plain, well-lit background works best).

#### Face tracking

**Follow face** keeps your face centred in the camera picture when you move. It needs a one-time analysis: press **Analyze face**. It runs on your Mac with the Vision framework, shows a progress bar with **Cancel**, and saves the result as `camera_track.json` in the project folder. It follows the largest face it finds. When it finishes, **Follow face** turns on. If it finds no face, the tab says `No face was found in the camera. Check that the camera shows you and try again.`

- **Zoom** (1× to 3×) sets how tight the picture is around the face. The default is 1.4× for the bubble and 1× in a split.
- Turning **Follow face** on before analyzing shows `Analyze the camera first.`
- Press **Analyze face** again to redo the analysis.

The analysis and `camera_track.json` stay on your Mac.

### Saving

The editor writes `project.json` by itself 1 s after your last change, as a temporary file renamed into place. The save state reads `Saved` or `Saving…`; if a write fails it shows the error text and tries again. A recording you only opened is not rewritten.

### Export…

**Export…** (or <kbd>⌘</kbd> <kbd>E</kbd>) opens a sheet. It saves `project.json` first, because the export renders the saved project.

| Format | Options |
|---|---|
| **MP4** | Codec **H.264** or **HEVC**. Resolution **Source**, **1080p** or **4K** (the long side is 1920 or 3840 px), never above 3840×2160. Frame rate **30 fps** or **60 fps**. Quality **Standard** or **High**. |
| **GIF** | Width **480 px**, **720 px** or **960 px**. Frame rate **10 fps** or **15 fps**. |

The sheet shows the result, for example `1920×1080 · 2:31 · 4530 frames`, and the destination. **Choose…** opens a save dialog. The default name is the project name with `.mp4` or `.gif`, next to the project folder; a number is added if that name exists. While it runs, a bar shows the percentage and the time left. **Cancel** (or <kbd>Esc</kbd>) stops it and leaves no partial file. At the end a toast says `Exported NAME` and the file is shown in Finder. Closing the editor cancels an export in progress. The exported length is the length of the edited timeline, trims and speed changes included.

A GIF holds all its frames in memory, so its length is limited. At 960 px wide (16:9, 15 fps) the limit is 20 seconds; a narrower or slower GIF can be longer, for example about 80 seconds at 480 px. Over the limit the sheet says:

`This edit is too long for a GIF: the limit is N seconds. Trim it or export an MP4.`

(N is the limit for the width, frame rate and aspect you chose.)

:::caution[Limits]
- PNG only. Settings → Capture shows the format as PNG, with no other choice. **Include cursor** draws the pointer in area, window and screen captures.
- The scrolling capture keeps all its strips in memory until it finishes.
- Audio is only in MP4 exports. Transcripts need the Speech Recognition permission and an on-device model for the language; words are cut, never retyped.
- The editor does not use image wallpapers: the Style presets are gradients.
- Recording effects (loupe, split screen, looks, cutout and face tracking) show in the preview and in MP4 and GIF exports only, and they slow an export a little. Cutout edges depend on the light and the background. Face tracking follows the largest face and needs the analysis first. Custom looks are set in `project.json` (there is no file picker).
- A recording is lost if you quit Ferrocast while it runs. Stop it first.
:::

## Related

- [Settings](/docs/launcher/settings/)
