---
title: "Troubleshooting"
description: "Fix common Ferrocast problems: the Accessibility prompt after an upgrade, the short brew command failing, Gatekeeper, the DMG, missing ffmpeg and the log."
sidebar:
  order: 3
---
## Accessibility is requested again after `brew upgrade`

The ad-hoc signature is anchored to the identifier `dev.ferrocast.launcher` so macOS should keep the permission between versions. That is not verified in a real session. If macOS asks again, grant it again. Recompiling with an ad-hoc signature can also make macOS ask again.

## `brew install drumst0ck/ferrocast/ferrocast` fails

The short form tries to clone `homebrew-ferrocast`, which does not exist. Use the long form with the URL: `brew tap drumst0ck/ferrocast https://github.com/drumst0ck/ferrocast`, then `brew install ferrocast`. See [Install](/docs/getting-started/install/).

## Gatekeeper blocks the DMG

The DMG from `scripts/dmg.sh` is not notarized, so Gatekeeper blocks it on other Macs. It is for your own use. Install through Homebrew instead, which builds on your machine so the app has no quarantine flag.

## “ffmpeg is not installed” (or LibreOffice, webp, Ghostscript, unrar)

An optional tool is missing. The message includes the install command, for example `brew install ffmpeg`. See [Optional tools](/docs/files/optional-tools/).

## The hotkey, snippets or window commands do nothing

They need Accessibility (snippets may also need Input Monitoring). Open the Welcome & Permissions screen or System Settings and grant it. See [First run](/docs/getting-started/first-run/).

## Convert Finder Selection shows a toast or nothing

The Finder read needs Automation permission for Finder; while the prompt is pending, the call gives up after 15 seconds with a toast instead of hanging. It has not been tested in a real session.

## Pressing Enter on a Finder file renames it

That is Finder's own behaviour. Ferrocast does not intercept Return; use Convert Finder Selection or <kbd>⌥</kbd> <kbd>↵</kbd> on a file result.

## Something crashed or failed silently

Run the **Reveal Log File** command in Ferrocast to open the error log, and read the last lines.

## Related

- [Frequently asked questions](/docs/reference/faq/)
- [Data and privacy](/docs/reference/data-and-privacy/)
