---
title: "Migrating from Raycast"
description: "What carries over when you move from Raycast to Ferrocast. Script Commands run as they are, snippets import from JSON, and what is not included yet."
sidebar:
  order: 4
---
Ferrocast is inspired by Raycast but is a separate project: it is not affiliated with Raycast and uses none of its brand or assets. This page is the only place the docs compare the two.

## What carries over

- **Script Commands** run as they are. Ferrocast reads Raycast-compatible metadata and supports the full, compact, silent and inline modes. See [Script Commands](/docs/launcher/script-commands/).
- **Snippets** import from a Raycast JSON export (Settings, backup and import). See [Snippets](/docs/launcher/snippets/) and [Settings](/docs/launcher/settings/).
- **Quicklinks** you recreate by hand; it takes a minute. See [Quicklinks](/docs/launcher/quicklinks/).

## What is not included

React extensions, sync, Search Menu Bar Items and AI context (selection or screen). There is no extension store.

## Side by side

| | Ferrocast | Raycast (as described on the home page) |
|---|---|---|
| Price | Free. MIT licence. | Free tier; Pro subscription for AI and cloud sync. |
| Account | None. | Optional; needed for Pro features. |
| Extensions | Script Commands (Raycast-compatible). No store. | Thousands in the Store. |
| Source code | Open. | Closed. |
| Install | Homebrew builds it from source on your Mac. | Signed, notarized download. |

"Raycast" is the name of a third-party product, used here only for comparison.

:::caution[Limits]
If you live inside Raycast extensions, Ferrocast is not a drop-in replacement yet. Script Command arguments are passed on the command line, as in Raycast, and can be seen with `ps` while the script runs.
:::

## Related

- [Script Commands](/docs/launcher/script-commands/)
- [Frequently asked questions](/docs/reference/faq/)
