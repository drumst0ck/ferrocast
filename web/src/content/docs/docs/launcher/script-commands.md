---
title: "Script Commands"
description: "Turn any script into a Ferrocast command. Raycast-compatible metadata, full, compact, silent and inline output modes, and confirmation for risky scripts."
status: built
sidebar:
  order: 8
---
Script Commands turn a script into a launcher command. Ferrocast reads Raycast-compatible metadata and supports four output modes: **full**, **compact**, **silent** and **inline**.

A minimal command:

```bash
#!/bin/bash
# @raycast.mode compact
date +%H:%M
```

Manage script folders in Settings, Scripts tab, and run the commands from the launcher by name. Scripts with `needsConfirmation` ask for confirmation before running, and the confirm view no longer stays under a script's output.

:::caution[Limits]
Script Command arguments (including those of type `password`) are passed on the command line, as in Raycast, and can be seen with `ps` while the script runs. Do not put critical secrets there.
:::

## Status

**Built.** Implemented for all four modes.

## Related

- [Migrating from Raycast](/docs/getting-started/migrating-from-raycast/)
- [Settings](/docs/launcher/settings/)
