---
title: "Migrating from Raycast"
description: "What carries over when you move from Raycast to Ferrocast. Script Commands run as they are, snippets import from JSON, and what is not included yet."
sidebar:
  order: 4
---
Ferrocast is independent and not affiliated with Raycast.

## What carries over

- Script Commands run as they are. Ferrocast reads Raycast-compatible metadata and supports the full, compact, silent and inline modes. See [Script Commands](/docs/launcher/script-commands/).
- Snippets import from a Raycast JSON export (Settings, backup and import). See [Snippets](/docs/launcher/snippets/) and [Settings](/docs/launcher/settings/).
- Quicklinks you recreate by hand; it takes a minute. See [Quicklinks](/docs/launcher/quicklinks/).

## What is not included

React extensions, sync, Search Menu Bar Items and AI context (selection or screen). There is no extension store.

## Limits

:::caution[Limits]
Script Command arguments are passed on the command line, as in Raycast, and can be seen with `ps` while the script runs.
:::

## Comparing options?

The [Raycast alternative](/raycast-alternative/) page sums up what you keep, what is missing, and a sourced table of Ferrocast, Raycast and Alfred.

## Related

- [Script Commands](/docs/launcher/script-commands/)
- [Frequently asked questions](/docs/reference/faq/)
