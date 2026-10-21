---
title: "First run and permissions"
description: "What Ferrocast asks for the first time you open it, why it needs Accessibility, Input Monitoring and Automation for Finder, and what works without them."
status: built
sidebar:
  order: 2
---
The first time you open Ferrocast it shows a **Welcome & Permissions** screen that walks you through the permissions it can use. Permissions are granted by you in System Settings; Ferrocast cannot grant them itself.

Open the launcher with <kbd>⌥</kbd> <kbd>Space</kbd> (change it in Settings).

## Permissions

| Permission | Needed for |
|---|---|
| Accessibility | Moving windows, pasting for you, expanding snippets, global hotkeys. |
| Input Monitoring | Features that watch keyboard input, such as snippet auto-expansion. macOS may ask for it together with Accessibility. |
| Automation for Finder | The Convert Finder Selection command, which reads what is selected in Finder. |

Search, the calculator and Ask AI work without Accessibility. The drag wheel needs no Accessibility permission either.

## What has been verified

- Verified: tests (core, bindings, UI state), starting the `.app`, and the real look of the window through internal screenshots (`scripts/shot.sh`): search, calculator, currency, time zones, clipboard with detail, the ⌘K panel, emoji and settings.
- Permissions (Accessibility / Input Monitoring) are granted by the user. After recompiling with an ad-hoc signature macOS may ask for them again. Distributing to other Macs would need Developer ID signing and notarization.

:::caution[Limits]
**Not verified in a real session**: focus when opening and closing, global hotkeys, pasting, snippet expansion and window management (they require Accessibility, which the user grants). The Finder read behind Convert Finder Selection needs Automation permission and has not been tested in a real session.
:::

## Status

**Built.** Implemented; the permission-dependent behaviour is not tried in a live session.

## Related

- [Install](/docs/getting-started/install/)
- [Window management](/docs/launcher/window-management/)
- [Snippets](/docs/launcher/snippets/)
- [Drag wheel](/docs/files/drag-wheel/)
