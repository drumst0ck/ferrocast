---
title: "First run and permissions"
description: "What Ferrocast asks for the first time you open it, why it needs Accessibility, Input Monitoring and Automation for Finder, and what works without them."
sidebar:
  order: 2
---
The first time you open Ferrocast it shows a nine-slide **welcome tour**: what the app does, including one slide on screen recording and one on the floating Notes window, then a slide that walks you through the permissions it can use. Use <kbd>←</kbd> <kbd>→</kbd> or <kbd>↵</kbd> to move, <kbd>esc</kbd> to skip; the **‹** **›** arrows and the dots at the bottom, and the **Next** and **Skip** chips in the footer, do the same with the mouse. Search for “Welcome” to see it again. Permissions are granted by you in System Settings; Ferrocast cannot grant them itself.

Open the launcher with <kbd>⌥</kbd> <kbd>Space</kbd> (change it in Settings).

## Permissions

| Permission | Needed for |
|---|---|
| Accessibility | Moving windows, pasting for you, expanding snippets, global hotkeys. |
| Input Monitoring | Features that watch keyboard input, such as snippet auto-expansion. macOS may ask for it together with Accessibility. |
| Automation for Finder | The Convert Finder Selection command, which reads what is selected in Finder. |

Search, the calculator and Ask AI work without Accessibility. The drag wheel needs no Accessibility permission either.

## Permissions and signing

Permissions (Accessibility / Input Monitoring) are granted by the user. After recompiling with an ad-hoc signature macOS may ask for them again. Distributing to other Macs would need Developer ID signing and notarization.

## Related

- [Install](/docs/getting-started/install/)
- [Window management](/docs/launcher/window-management/)
- [Snippets](/docs/launcher/snippets/)
- [Drag wheel](/docs/files/drag-wheel/)
