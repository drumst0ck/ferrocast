<p align="center"><img src="web/public/icon.png" width="96" alt="Ferrocast icon"></p>

# Ferrocast

Fast, open-source launcher for macOS, written in Rust. Search, calculate, paste and ask AI from the keyboard; drag any file, tap a key, and convert, compress, crop, trim or redact it on your Mac. Nothing is uploaded.

[![CI](https://github.com/drumst0ck/ferrocast/actions/workflows/ci.yml/badge.svg)](https://github.com/drumst0ck/ferrocast/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[Website](https://ferrocast.app) · [Docs](https://ferrocast.app/docs/) · [Changelog](https://ferrocast.app/changelog/) · [Español](README.es.md)

<p align="center"><img src="web/public/img/home.jpg" alt="Ferrocast launcher" width="46%"> <img src="web/public/img/files-wheel-convert.jpg" alt="Ferrocast conversion wheel" width="46%"></p>

## Install

```bash
brew tap drumst0ck/ferrocast https://github.com/drumst0ck/ferrocast
brew install ferrocast                      # builds on your Mac (needs Rust, Homebrew installs it)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # grant Accessibility when asked
brew upgrade ferrocast                      # to update
```

**Why a formula and not a cask.** Homebrew's official cask requires signed, notarized apps (a paid Apple Developer account). Building from source on your own Mac means the app carries no quarantine flag and Gatekeeper does not step in: no paid account and no protection bypassed. The first install takes a few minutes because it compiles, and the signature is ad-hoc, for local use.

Always use the full `brew tap` with the URL: the short form `brew install drumst0ck/ferrocast/ferrocast` tries to clone `homebrew-ferrocast`, which does not exist. Apple Silicon only. More in the [install guide](https://ferrocast.app/docs/getting-started/install/).

## What it does

- **Search** apps, commands and files with frecency, aliases, favorites and a hotkey per command.
- **Calculate** in the search box: math, percentages, units, currencies (ECB rates) and time zones.
- **Clipboard history** for text, files and images; password-manager entries are skipped.
- **Snippets** with placeholders and auto-expansion, and **quicklinks** with arguments.
- **Script Commands** compatible with Raycast metadata (full, compact, silent, inline).
- **Window management** with about 40 layouts, plus Kill Process, timers, notes, emoji and Quick Look.
- **Ask AI** with Ollama, any OpenAI-compatible server, the Anthropic API, Claude Code, or Sign in with ChatGPT.

## File tools

- Drag a file, then tap <kbd>⇧</kbd> for conversions, <kbd>⌘</kbd> for tools or <kbd>⌃</kbd> for actions.
- **275 conversions** across 46 input formats (images, audio, video, documents, spreadsheets, PDF, slides, subtitles, archives).
- **26 file tools**: compress, crop, trim, split, merge, redact, annotate, edit metadata, organize PDF pages and more.
- **7 file actions**: AirDrop, shelf, copy, move, copy path, Terminal and Zip.
- Local only. macOS tools come first; ffmpeg, LibreOffice, cwebp, unrar and Ghostscript are optional. Results go next to the original and never overwrite it.

Browse every pair at <https://ferrocast.app/files/>.

## Ask AI and privacy

Out of the box Ask AI talks to Ollama on your own Mac. You can point it at LM Studio, OpenAI or the Anthropic API (keys go to the Keychain), or use Claude Code with your Claude plan. Sign in with ChatGPT uses the same flow as OpenAI's Codex CLI; OpenAI has not blessed third-party use of it, so the API with a key is the by-the-book route.

Data lives in `~/Library/Application Support/Ferrocast` as plain, unencrypted JSON (`FERROCAST_DATA_DIR` changes the path).

## Comparison

Not included: React extensions, sync, Search Menu Bar Items and AI context from selection or screen.

Ferrocast is inspired by Raycast but is not a replica and uses no Raycast brand or assets. It is free and MIT-licensed, needs no account, keeps data in JSON on your disk and has no extension store. Raycast does far more; see the measured, honest comparison at <https://ferrocast.app/#why>. Raycast is a trademark of its owner.

## Build from source

```bash
scripts/bundle.sh          # builds dist/Ferrocast.app (ad-hoc signed, local use only)
scripts/dmg.sh             # dist/Ferrocast-<version>.dmg
open dist/Ferrocast.app    # default hotkey: ⌥ Space
cargo test --workspace
```

Rust 1.95 is pinned in `rust-toolchain.toml` (egui 0.36 requires it). The unnotarized DMG is blocked by Gatekeeper on other Macs; the supported route is Homebrew. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Documentation

[ferrocast.app/docs](https://ferrocast.app/docs/) · [Changelog](https://ferrocast.app/changelog/) ([RSS](https://ferrocast.app/changelog/rss.xml)) · [Feature research](docs/RESEARCH.md)

## License

MIT, see [LICENSE](LICENSE).
