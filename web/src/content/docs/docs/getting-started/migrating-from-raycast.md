---
title: "Migrating from Raycast"
description: "What carries over from Raycast: Script Commands, snippets, partial extensions you already installed, and opt-in iCloud sync."
sidebar:
  order: 4
---
Ferrocast is independent and not affiliated with Raycast.

## What carries over

- Script Commands run as they are. Ferrocast reads Raycast-compatible metadata and supports the full, compact, silent and inline modes. See [Script Commands](/docs/launcher/script-commands/).
- Snippets import from a Raycast JSON export (Settings, backup and import). See [Snippets](/docs/launcher/snippets/) and [Settings](/docs/launcher/settings/).
- Quicklinks you recreate by hand; it takes a minute. See [Quicklinks](/docs/launcher/quicklinks/).
- Extensions you already installed with Raycast can run when you turn on [Run Raycast extensions](/docs/launcher/raycast-extensions/). It is off by default, needs Node.js 22 or newer, and does not contact Raycast’s Store. Not every part works.
- [Sync with iCloud](/docs/launcher/sync/) is opt-in (Settings → Advanced, off by default). It is not Raycast Cloud Sync.

## What is not included

An extension store of its own, Search Menu Bar Items, and AI context (selection or screen). Menu bar commands, OAuth sign-in, AI features, the file picker and the link accessory are not supported yet.

## Limits

:::caution[Limits]
Script Command arguments are passed on the command line, as in Raycast, and can be seen with `ps` while the script runs.
:::

## Comparing options?

The [Raycast alternative](/raycast-alternative/) page sums up what you keep, what is missing, and a sourced table of Ferrocast, Raycast and Alfred.

## Related

- [Script Commands](/docs/launcher/script-commands/)
- [Frequently asked questions](/docs/reference/faq/)
