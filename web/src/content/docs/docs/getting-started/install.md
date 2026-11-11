---
title: "Install Ferrocast"
description: "Download the Ferrocast DMG for Apple Silicon or Intel and drag it to Applications, or install it with Homebrew as an alternative that builds on your Mac."
sidebar:
  order: 1
---
## Download the DMG

[Download the latest DMG](https://github.com/drumst0ck/ferrocast/releases/latest/download/Ferrocast.dmg), open it, drag Ferrocast to Applications and open it. The app is signed ad-hoc, not notarized, so macOS blocks the first launch of a browser download: open System Settings → Privacy & Security and choose “Open Anyway”. Every release, with a `SHA256SUMS.txt`, is on the [GitHub Releases page](https://github.com/drumst0ck/ferrocast/releases). Apple Silicon and Intel (universal; Intel not tested on real hardware).

## Alternative: install with Homebrew

The Homebrew formula compiles the app from source on your own Mac, so the app carries no quarantine flag and macOS shows no warning. Homebrew installs Rust for you if you do not have it, and the first install takes a few minutes. Run these commands in Terminal:

```bash
brew tap drumst0ck/ferrocast
brew install drumst0ck/ferrocast/ferrocast  # compiles on your Mac (needs Rust, brew installs it)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # grant Accessibility when it asks
brew upgrade ferrocast                      # to update
```

Run `brew tap` first: Homebrew refuses to load formulae from a tap you have not added. The formula lives in [drumst0ck/homebrew-ferrocast](https://github.com/drumst0ck/homebrew-ferrocast) and follows each stable release automatically.

## Updates

- **Download install**: Ferrocast checks GitHub about once an hour for a new stable release (prereleases are ignored), shortly after launch, and again when you open the launcher if the last check is more than 15 minutes old. A failed check is retried after 15 minutes. When a newer version exists, the root search bar gets an accent outline and the Ferrocast logo with “Update to {version}” at its right end; click it (or press ⌘U) to install right away. Nothing pops up on its own. You can switch to automatic installs, turn the automatic check off, or press “Check now” under Settings → General → Updates; the tray menu and About also have “Check for Updates…”, which opens the release notes so you can install, postpone or skip that version. The indicator is not shown for a skipped version, for development builds, or when the app folder is not writable.
- **Homebrew install**: the app never replaces itself; the indicator and Install run `brew upgrade ferrocast`.
- An automatic update waits while a recording or an export is running and tries again at the next check; the app restarts only after it ends.
- A self-update replaces the app with the new build. Because the signature is ad-hoc, macOS may ask for Accessibility again after it; grant it again.

## Why a formula and not a cask

The official Homebrew cask requires signed and notarized apps (a paid Apple Developer account) and disables the ones that do not pass Gatekeeper from September 2026. Building from source on your own machine means the app carries no quarantine flag and Gatekeeper does not get involved: no paid account is needed and no protection is bypassed. In exchange, the first install compiles for a few minutes and the signature is ad-hoc, for local use.

## Update

`brew upgrade ferrocast`. The ad-hoc signature is anchored to the identifier `dev.ferrocast.launcher` instead of the binary hash, so macOS should keep the Accessibility permission between versions. If macOS asks again after an upgrade, grant it again. See [Troubleshooting](/docs/reference/troubleshooting/).

## Uninstall

`brew uninstall ferrocast` removes the formula; delete the `/Applications/Ferrocast.app` symlink created above. Your data lives in `~/Library/Application Support/Ferrocast` (see [Data and privacy](/docs/reference/data-and-privacy/)); delete that folder yourself if you want it gone.

:::caution[Limits]
The DMG/zip runs on Apple Silicon and Intel Macs (one universal download; Intel is not tested on real hardware). Homebrew builds from source for your Mac's own architecture. The DMG is not notarized.
:::

## Related

- [First run](/docs/getting-started/first-run/)
- [Build from source](/docs/getting-started/build-from-source/)
- [Troubleshooting](/docs/reference/troubleshooting/)
