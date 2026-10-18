# File tools (Tangerine-style converter and file actions) in Ferrocast
Branch: feat/ui-polish-and-fixes · Owner: parent session · Status: in progress
Goal: bring Tangerine for Mac's capabilities into Ferrocast, local-only, same interaction model (drag + key → wheel), without its brand or assets.

## Specs
- S1 User request (verbatim, L1): "vamos a incluir en nuestra app las funcionalidades de este programa https://tangerineformac.com/ todas, y de la misma manera, basicamente la copiaremos para incluirla y hacer nuestro ferrocat mas potente".
- S2 Tangerine's model (from its site, 2026-10-18): "Drag a file, tap Shift, and drop. Turn files into the formats you need without breaking your flow." "Everything happens locally on your Mac." "Start dragging, then tap a key. The wheel stays open until you drop. Change the keys in Settings."
- S3 Conversions: "Across images, video, audio, documents, spreadsheets, and archives." Formats listed: Images "JPG PNG WebP HEIC TIFF AVIF BMP"; Audio "MP3 M4A WAV FLAC OGG Opus AIFF WMA"; Video "MP4 MOV MKV WebM AVI WMV GIF"; PDF "PDF → DOCX PAGES PPTX XLSX JPG PNG EPUB TXT MD"; Documents "DOCX PAGES Markdown TXT EPUB"; Slides "PPT / PPTX → PDF JPG PNG"; Spreadsheets "XLSX ODS CSV TSV"; Subtitles "SRT VTT"; Archives "ZIP TAR GZIP".
- S4 File tools: "Compress, crop, trim, split, merge, and more." Image tools: Compress images ("Choose a target file size, percentage reduction, or preset. Resize while compressing"), Edit metadata ("Edit camera and location tags, or remove metadata"), Edit photos, Annotate photos, Add a background, Crop images ("Choose an aspect ratio or enter exact output dimensions"), Redact photos ("Detect faces or text locally, then cover"), Create a PDF ("Combine selected images into one PDF, with a page per image"), Make a collage.
- S5 File actions ("Drag any file or folder, then tap Control."): AirDrop, Float ("Keep files in a floating shelf. Pin it to a screen edge"), Copy ("Copy files to the clipboard, ready to paste in Finder"), Move ("Choose a destination folder and move the originals there"), Copy Path ("one path per line for a selection"), Terminal ("Open a folder, or a file’s containing folder, in Terminal or iTerm2"). "Works with folders, mixed selections, and files that have no conversion tools."
- S6 Engine constraint (decision): conversions run with tools present on every Mac (sips, afconvert, avconvert, textutil, ditto/zip/tar) and use ffmpeg / LibreOffice only when installed, with a clear hint otherwise. Nothing is uploaded.
- S7 Output: next to the original, same base name, new extension; never overwrite (append " 2", " 3"…). Toast with result and a Reveal action.

## Tasks
- T1 (S3, S6, S7) Converter engine in core: format catalogue, target list per input, tool planning, output naming. Unit-tested. Route: inline.
- T2 (S3, S7) Launcher UI: drop files on the window or pick a file result → "Convert to…" view → run async → toast + reveal. Route: inline.
- T3 (S4) Image tools v1: Compress (quality/size presets, resize), Crop (aspect or exact), Strip metadata, Images → PDF. Route: inline.
- T4 (S5) File actions on any file/folder item: Copy, Move to…, Copy Path(s), Open in Terminal/iTerm2, AirDrop. Route: inline.
- T5 (S2) Drag + Shift wheel: global drag detection, drag pasteboard inspection, drop overlay viewport with radial menu; Control → actions wheel; keys in Settings. Route: inline, after T1–T4.
- T6 (S3, S6) Breadth: audio/video/docs/sheets/subtitles/archives matrix incl. optional ffmpeg/soffice; tool hints. Route: inline.
- T7 (S4) Image editors: Annotate, Add a background, Collage, Edit photos, Redact (Vision). Route: later; needs its own UI.
- T8 (S5) Float shelf window. Route: later.
- T9 Docs: README, CHANGELOG, website section. Route: inline with each task.

## Log
- L1 2026-10-18 User: "vamos a incluir en nuestra app las funcionalidades de este programa https://tangerineformac.com/ todas, y de la misma manera, basicamente la copiaremos para incluirla y hacer nuestro ferrocat mas potente".
- L2 Tool survey on this Mac: sips writes pdf gif bmp avif heic jpeg jp2 png tiff (reads webp, heif, jxl); avconvert presets incl. PresetHighestQuality, Preset1280x720/1920x1080/3840x2160, HEVC*, AppleM4A, AppleM4V*; afconvert formats m4af/AIFF/WAVE/caff/adts…; textutil converts txt rtf rtfd html doc docx odt webarchive. ffmpeg at /opt/homebrew/bin/ffmpeg and soffice at /opt/homebrew/bin/soffice are present here but not on every Mac (S6).
