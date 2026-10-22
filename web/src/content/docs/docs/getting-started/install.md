---
title: "Install with Homebrew"
description: "Install Ferrocast on Apple Silicon with Homebrew. The formula compiles the app on your Mac, so there is no quarantine flag and no paid developer account."
sidebar:
  order: 1
---
Ferrocast installs through a Homebrew formula that compiles the app from source on your own Mac. Homebrew installs Rust for you if you do not have it. The first install takes a few minutes.

Run these commands in Terminal:

```bash
brew tap drumst0ck/ferrocast
brew install drumst0ck/ferrocast/ferrocast  # compiles on your Mac (needs Rust, brew installs it)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # grant Accessibility when it asks
brew upgrade ferrocast                      # to update
```

Run `brew tap` first: Homebrew refuses to load formulae from a tap you have not added. The formula lives in [drumst0ck/homebrew-ferrocast](https://github.com/drumst0ck/homebrew-ferrocast) and follows each stable release automatically.

## Download the DMG

[Download the latest DMG](https://github.com/drumst0ck/ferrocast/releases/latest/download/Ferrocast.dmg). Every stable release is also published on the [GitHub Releases page](https://github.com/drumst0ck/ferrocast/releases) as `Ferrocast-X.Y.Z.dmg` with a `SHA256SUMS.txt`. Open the DMG, drag Ferrocast to Applications and open it. The app is signed ad-hoc, not notarized, so macOS blocks the first launch of a browser download: open System Settings → Privacy & Security and choose “Open Anyway”. Apple Silicon only.

## Updates

- **Download install**: Ferrocast checks GitHub once a day for a new stable release (prereleases are ignored). By default it shows the release notes and lets you install, postpone or skip that version; you can switch to automatic installs, turn the daily check off, or press “Check now” under Settings → General → Updates. The tray menu and About also have “Check for Updates…”.
- **Homebrew install**: the app never replaces itself; updates run `brew upgrade ferrocast`.
- A self-update replaces the app with the new build. Because the signature is ad-hoc, macOS may ask for Accessibility again after it; grant it again.

## Why a formula and not a cask

The official Homebrew cask requires signed and notarized apps (a paid Apple Developer account) and disables the ones that do not pass Gatekeeper from September 2026. Building from source on your own machine means the app carries no quarantine flag and Gatekeeper does not get involved: no paid account is needed and no protection is bypassed. In exchange, the first install compiles for a few minutes and the signature is ad-hoc, for local use.

## Update

`brew upgrade ferrocast`. The ad-hoc signature is anchored to the identifier `dev.ferrocast.launcher` instead of the binary hash, so macOS should keep the Accessibility permission between versions. If macOS asks again after an upgrade, grant it again. See [Troubleshooting](/docs/reference/troubleshooting/).

## Uninstall

`brew uninstall ferrocast` removes the formula; delete the `/Applications/Ferrocast.app` symlink created above. Your data lives in `~/Library/Application Support/Ferrocast` (see [Data and privacy](/docs/reference/data-and-privacy/)); delete that folder yourself if you want it gone.

:::caution[Limits]
Ferrocast needs Apple Silicon; Intel Macs are not supported. The DMG is not notarized.
:::

## Related

- [First run](/docs/getting-started/first-run/)
- [Build from source](/docs/getting-started/build-from-source/)
- [Troubleshooting](/docs/reference/troubleshooting/)
