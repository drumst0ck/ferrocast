---
title: "Clipboard history"
description: "Ferrocast keeps a local clipboard history of text, links, colours, files and images, lets you pin entries, and skips entries copied from password managers."
sidebar:
  order: 5
---
Clipboard history records text, files and images you copy, with links and colours recognised. You can pin entries. Anything copied from a password manager such as 1Password is skipped on purpose.

Open the Clipboard view from the launcher. Each entry has a detail pane with an Information list (type, source, when, times, length, pinned). <kbd>⌘</kbd> <kbd>.</kbd> pins or unpins an entry and <kbd>⌃</kbd> <kbd>X</kbd> deletes it.

You can do the same with the mouse. Hover a row for its **Pin** (or **Unpin**) and **Delete** buttons, or right-click the row to open the action panel. **Clear History…** at the top right deletes every entry after a confirmation; pinned entries stay.

## Storage and privacy

History stays on your Mac, in a JSON file in Application Support (unencrypted; see [Data and privacy](/docs/reference/data-and-privacy/)). Deleted clipboard images are removed from disk. A malformed `file://` URL on the pasteboard cannot panic the clipboard watcher. Backup export leaves the clipboard history out.

:::caution[Limits]
Pasting requires Accessibility.
:::

## Related

- [Data and privacy](/docs/reference/data-and-privacy/)
- [Settings](/docs/launcher/settings/)
