---
title: "Snippets and placeholders"
description: "Create Ferrocast snippets that expand as you type, with placeholders such as date, clipboard, cursor and snippet name, and import Raycast snippet JSON."
status: built
sidebar:
  order: 6
---
A snippet is a keyword that expands into longer text as you type it, in any app. Snippets support dynamic placeholders and automatic expansion; you can also paste them from the launcher.

Open Snippets from the launcher; <kbd>⌘</kbd> <kbd>N</kbd> creates one and <kbd>⌘</kbd> <kbd>E</kbd> edits the selected one. Raycast snippet JSON exports can be imported (see [Settings](/docs/launcher/settings/)).

## Placeholders

`{date}`, `{clipboard}`, `{cursor}` and `{snippet name=…}` (which inserts another snippet). `{date offset=…}` no longer panics on absurd values, and `{snippet name=…}` also resolves in pastes and quicklinks.

## Keyword rules

Snippet keywords that prefix another keyword are rejected. A held <kbd>⌫</kbd> no longer closes the launcher.

:::caution[Limits]
Automatic expansion needs permissions (Accessibility; Input Monitoring may also be requested) and is **not verified in a real session**.
:::

## Status

**Built.** Implemented; expansion was not proven in a live session.

## Related

- [First run and permissions](/docs/getting-started/first-run/)
- [Quicklinks](/docs/launcher/quicklinks/)
