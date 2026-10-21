---
title: "Typed commands"
description: "Every Ferrocast typed file command in one place: collage, frame, edit, redact, compress, crop, metadata, trim, split, speed, normalize, bleep, pages."
status: typed
sidebar:
  order: 5
---
Typed commands skip the tool window: type a short command in the Convert view for a file and press <kbd>↵</kbd>. Results are written next to the original and never overwrite it.

## Image

| Command | Effect |
|---|---|
| `500kb`, `1.5mb`, `40%` | Compress to a size or percent reduction. |
| `800x600` | Exact crop. |
| `collage row 1600 gap 16 radius 12 bg 000000` | Collage (grid, row, column, featured). |
| `frame blur 40 pad 80 shadow 30` | Background: colour or blurred backdrop, padding, corners, shadow. |
| `frame image /path/bg.jpg` | Use another picture as the background. |
| `edit brightness 20 contrast 10 saturation -30 warmth 15 clarity 40 sharpen 30 grain 20 denoise 30 dehaze 50` | One-step photo adjustments. |
| `redact faces`, `redact text blur 25` | Redact faces or text on-device with Vision. |
| `redact 10,10 200x100 300,40 80x80 color ff0000` | Redact typed areas with a colour. |
| `redact 10,10 200x100 blur` | Redact an area with blur. |
| `make Canon`, `model EOS R5`, `gps 40.4,-3.7` | Camera and GPS tags (JPEG). |

## Audio, video and PDF

| Command | Effect |
|---|---|
| `trim 0:10-0:30` | Trim audio or video. |
| `split 30s` | Split audio or video. |
| `mute` | Mute a video. |
| `speed 1.5x` | Change video speed. |
| `frame 1:23` | Snapshot at a time. |
| `crop 640x360 at 100,50` | Crop a video frame. |
| `redact 10,10 200x100 0:05-0:09 blur` | Redact an area of a video for a time range. |
| `normalize -14` | Normalize loudness (LUFS). |
| `mono`, `stereo`, `silence` | Channels, and trim silence at the ends. |
| `bleep 0:05-0:07` | Replace a range with a 1 kHz tone. |
| `visualizer square image /path/cover.jpg` | Audio to video. |
| `pages 3,1,2r` | Reorder PDF pages (`r` rotates). |

Join accepts several clips; silent clips get generated silence.

:::caution[Limits]
Typed audio, video and PDF tools are tested with real files and were reviewed by an independent verifier with no blockers. Normalize is single pass and shows the measured loudness before and after (for example `-31.0 → -16.0 LUFS`, measured with ffmpeg's ebur128), with no waveform or compare view; WMV fails on mute and crop. Typed image tools are tested with real files except faces, which have no test with a real photo of a face. PNG tags are not supported for camera and GPS commands (a clear error is shown). Trim and split use stream copy, so video cuts land on keyframes.
:::

## Status

**Typed.** These are implemented as typed commands and tested with real files; live-preview windows exist for several of them (see each [tool page](/docs/files/tools/compress/)).

## Related

- [Optional tools](/docs/files/optional-tools/)
- [Conversions](/docs/files/conversions/)
