---
title: "Install with Homebrew"
description: "Install Ferrocast on Apple Silicon with Homebrew. The formula compiles the app on your Mac, so there is no quarantine flag and no paid developer account."
status: tested
sidebar:
  order: 1
---
Ferrocast installs through a Homebrew formula that compiles the app from source on your own Mac. Homebrew installs Rust for you if you do not have it. The first install takes a few minutes.

Run these commands in Terminal:

```bash
brew tap drumst0ck/ferrocast https://github.com/drumst0ck/ferrocast
brew install ferrocast                      # compiles on your Mac (needs Rust, brew installs it)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # grant Accessibility when it asks
brew upgrade ferrocast                      # to update
```

Always use the `brew tap` form with the URL. The short form `brew install drumst0ck/ferrocast/ferrocast` tries to clone `homebrew-ferrocast`, which does not exist, and fails.

## Why a formula and not a cask

The official Homebrew cask requires signed and notarized apps (a paid Apple Developer account) and disables the ones that do not pass Gatekeeper from September 2026. Building from source on your own machine means the app carries **no quarantine flag** and Gatekeeper does not get involved: no paid account is needed and no protection is bypassed. In exchange, the first install compiles for a few minutes and the signature is ad-hoc, for local use.

## Update

`brew upgrade ferrocast`. The ad-hoc signature is anchored to the identifier `dev.ferrocast.launcher` instead of the binary hash, so macOS should keep the Accessibility permission between versions. If macOS asks again after an upgrade, grant it again. See [Troubleshooting](/docs/reference/troubleshooting/).

## Uninstall

`brew uninstall ferrocast` removes the formula; delete the `/Applications/Ferrocast.app` symlink created above. Your data lives in `~/Library/Application Support/Ferrocast` (see [Data and privacy](/docs/reference/data-and-privacy/)); delete that folder yourself if you want it gone.

:::caution[Limits]
The install from the public tap (`brew tap` with the URL plus `brew install ferrocast`) was tested end to end: it compiles in about 2.5 minutes, the app has no quarantine flag, `codesign --verify --strict` passes and it starts through LaunchServices. Keeping the Accessibility permission across upgrades is **not verified in a real session**. Ferrocast needs Apple Silicon; Intel Macs are not supported. There is no notarized download, and the DMG is not a public download.
:::

## Status

**Tested** for the install path described above; the permission behaviour on upgrade is not verified.

## Related

- [First run](/docs/getting-started/first-run/)
- [Build from source](/docs/getting-started/build-from-source/)
- [Troubleshooting](/docs/reference/troubleshooting/)
