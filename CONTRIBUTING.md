# Contributing

## Setup

- macOS on Apple Silicon. Rust 1.95 is pinned in `rust-toolchain.toml` (egui 0.36 requires it).
- `cargo test --workspace` must pass. CI also runs `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings`.

## Layout

| Crate | Purpose |
|---|---|
| `crates/core` | Platform-independent logic: search, calculator, placeholders, scripts, storage. Pure and unit tested. |
| `crates/macos` | AppKit, Accessibility, CoreGraphics and pasteboard bindings. |
| `crates/app` | The egui launcher, background services and UI-state tests. |

## Working on the UI without screen permission

`scripts/shot.sh out.png FERROCAST_VIEW=settings` renders the window to a PNG from inside egui.
Supported inputs: `FERROCAST_VIEW` (clipboard, snippets, quicklinks, emoji, settings, notes, ai,
welcome, shortcuts, about), `FERROCAST_QUERY`, `FERROCAST_PANEL=1`. Use a throwaway
`FERROCAST_DATA_DIR` so real data is never touched.

## Rules of thumb

- Put logic in `core` with tests; keep `app` thin.
- Anything destructive must go through the confirmation view.
- Never put secrets in argv or logs. API keys live in the Keychain.
- Hotkeys, pasting, focus and snippet expansion need Accessibility and cannot run in CI: describe
  how you checked them by hand in the pull request.

## Releasing (Homebrew, no paid Apple account)

1. Bump `version` in the root `Cargo.toml`, update `CHANGELOG.md`, make sure CI is green.
2. `git tag vX.Y.Z && git push --tags`.
3. `scripts/update_formula.sh drumst0ck/ferrocast` rewrites `Formula/ferrocast.rb` (url + sha256) from the pushed tag; commit and push it.
4. Users run `brew upgrade ferrocast`.

The formula compiles from source on the user's machine, so the app has no quarantine flag and needs no
Developer ID. `scripts/notarize.sh` exists only for anyone who has a paid account and wants a signed DMG.
