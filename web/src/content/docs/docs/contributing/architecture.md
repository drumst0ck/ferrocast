---
title: "Architecture"
description: "How Ferrocast is organised: the core, macos and app crates, the egui launcher, and how a file conversion flows through matrix, convert, route and services."
sidebar:
  order: 2
---
Ferrocast is a Cargo workspace of three crates. The rule of thumb is to put logic in `core` with tests and keep `app` thin.

## Crates

- **`crates/core`**: platform-independent, pure and unit-tested logic: search and ranking, the calculator, placeholders, scripts, storage, and the file-tool planning (`matrix.rs` for the conversion table, `convert.rs` for tools and output naming, `tools.rs` for the 26 tools per format, `wheel.rs` for wheel geometry and key tracking, `compress.rs`, `videoplan.rs`, `waveform.rs`, `reorder.rs` and others).
- **`crates/macos`**: bindings to AppKit, Accessibility, CoreGraphics and the pasteboard, plus the drag watcher, the native shelf panel, PDF helpers and Vision.
- **`crates/app`**: the egui launcher (`app.rs`, `draw.rs`), background services (`services.rs`), the executors that run external tools (`route.rs`, `media_tools.rs`) and the UI-state tests.

## Data flow of a file conversion

1. `core/matrix.rs` holds the table of 46 input formats and 275 pairs and picks which tool is needed for each pair.
2. `core/convert.rs` plans the job: which engine (a macOS tool first, or optional ffmpeg, LibreOffice, cwebp, Ghostscript, unrar), the output name next to the original (never overwriting) and the install hint when a tool is missing.
3. `app/route.rs` and `app/media_tools.rs` run the engine as a child process with a deadline and read its output.
4. `app/services.rs` runs the job off the UI thread and reports the result back to the launcher as a toast with a Reveal action.

## Website data

`web/src/data/conversions.json` and `tools.json` are generated from the Rust code with `cargo run -p ferrocast-core --example site_data`; the status vocabulary in `web/src/data/status.ts` is hand-written and throws at build time when the counts drift from 275 and 46. The docs reference pages are generated from those JSON files by `npm run gen:docs`.

## Related

- [Contributing](/docs/contributing/)
- [Conversions](/docs/files/conversions/)
