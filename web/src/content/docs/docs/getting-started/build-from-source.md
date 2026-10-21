---
title: "Build from source"
description: "Build the Ferrocast app bundle and a DMG from source with the repository scripts. Requires Rust 1.95 and a Mac with Apple Silicon."
sidebar:
  order: 3
---
You can build Ferrocast without Homebrew from a checkout of the repository. This is also how contributors run it.

```bash
scripts/bundle.sh          # builds and creates dist/Ferrocast.app with icon (ad-hoc signed, local only)
scripts/dmg.sh             # dist/Ferrocast-<version>.dmg
open dist/Ferrocast.app    # default shortcut: ⌥ Space; the first time it shows Welcome & Permissions
cargo test --workspace     # about 70 tests
scripts/shot.sh out.png FERROCAST_QUERY="100 usd in eur"   # render the UI to a PNG (no screen permission needed)
scripts/perf.sh            # performance baseline table (startup, idle, views); FERROCAST_PROFILE=1 prints slow frames
```

It requires Rust 1.95 (pinned in `rust-toolchain.toml`; egui 0.36 requires it) and macOS on Apple Silicon. Data lives in `~/Library/Application Support/Ferrocast`; `FERROCAST_DATA_DIR` changes the path. See [Environment variables](/docs/reference/environment-variables/).

## Distribution

The supported route is Homebrew compiling from source. The unnotarized DMG from `scripts/dmg.sh` will be blocked by Gatekeeper on other Macs that download it, so it is only for your own use. Notarization (`scripts/notarize.sh`) is optional and requires a paid developer account.

:::caution[Limits]
The notarization script is optional. Intel Macs are not supported.
:::

## Related

- [Install with Homebrew](/docs/getting-started/install/)
- [Contributing](/docs/contributing/)
