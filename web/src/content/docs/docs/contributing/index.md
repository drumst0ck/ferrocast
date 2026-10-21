---
title: "Contributing"
description: "How to build, test and contribute to Ferrocast: setup, crate layout, working on the UI without screen permission, rules of thumb and Homebrew releases."
sidebar:
  order: 1
---
Ferrocast is an open Rust project. This page summarises `CONTRIBUTING.md`.

## Setup

- macOS on Apple Silicon. Rust 1.95 is pinned in `rust-toolchain.toml` (egui 0.36 requires it).
- `cargo test --workspace` must pass (about 70 tests). CI also runs `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`.

## Layout

| Crate | Purpose |
|---|---|
| `crates/core` | Platform-independent logic: search, calculator, placeholders, scripts, storage. Pure and unit tested. |
| `crates/macos` | AppKit, Accessibility, CoreGraphics and pasteboard bindings. |
| `crates/app` | The egui launcher, background services and UI-state tests. |

See [Architecture](/docs/contributing/architecture/).

## Working on the UI without screen permission

`scripts/shot.sh out.png FERROCAST_VIEW=settings` renders the window to a PNG from inside egui. Supported inputs: `FERROCAST_VIEW` (clipboard, snippets, quicklinks, emoji, settings, notes, ai, welcome, shortcuts, about), `FERROCAST_QUERY` and `FERROCAST_PANEL=1`. Use a throwaway `FERROCAST_DATA_DIR` so real data is never touched.

Other scripts: `scripts/perf.sh` prints a performance baseline (`FERROCAST_PROFILE=1` prints slow frames) and `scripts/gallery.sh` regenerates the website gallery pictures from the app's own screenshot mode on sample files (macOS only; it needs cargo, ffmpeg, ffprobe, sips, cupsfilter and perl).

## Rules of thumb

- Put logic in `core` with tests; keep `app` thin.
- Anything destructive must go through the confirmation view.
- Never put secrets in argv or logs. API keys live in the Keychain.
- Hotkeys, pasting, focus and snippet expansion need Accessibility and cannot run in CI: describe how you checked them by hand in the pull request.
- Pull requests are chained by topic (`chain/NN-topic` branches) and kept to roughly 400 authored lines, with tests and docs in the same change.

## Releasing (Homebrew, no paid Apple account)

1. Bump `version` in the root `Cargo.toml`, update `CHANGELOG.md`, make sure CI is green.
2. Tag the release (`vX.Y.Z`) and push the tag.
3. `scripts/update_formula.sh drumst0ck/ferrocast` rewrites `Formula/ferrocast.rb` (url + sha256) from the pushed tag; commit it and publish it.
4. Users run `brew upgrade ferrocast`.

The formula compiles from source on the user's machine, so the app has no quarantine flag and needs no Developer ID. `scripts/notarize.sh` exists only for anyone with a paid account who wants a signed DMG.

## Related

- [Build from source](/docs/getting-started/build-from-source/)
- [Environment variables](/docs/reference/environment-variables/)
