---
title: "The drag wheel"
description: "Drag files and tap Shift for conversions, Command for file tools or Control for file actions. The Ferrocast wheel stays open until you drop or cancel."
sidebar:
  order: 2
---
The drag wheel is a ring of choices that opens under the pointer while you drag files. There are three wheels, each with one accent colour: **Convert** (orange), **Tools** (blue) and **Actions** (green). No Accessibility permission is needed to open it.

Start dragging files, then tap <kbd>⇧</kbd> for conversions, <kbd>⌘</kbd> for file tools or <kbd>⌃</kbd> for actions. Release over a segment to run it; the wheel stays open until you drop or cancel. Tapping another key switches an open wheel. You can also open it from a file result with <kbd>⌥</kbd> <kbd>↵</kbd>; arrow keys choose, <kbd>↵</kbd> applies and <kbd>esc</kbd> cancels. The keys and an on/off switch are in Settings → General.

## Design

A halo ring: one accent per wheel, a gradient sector with an edge glow, a centre showing the wheel name, the file count and kind, and every segment showing its name under the icon (from nine segments the text is smaller and long names are cut with an ellipsis; the hub shows the full name of the hovered one). The Actions wheel has the six file actions plus Zip; the Convert wheel shows every output, dimming the ones whose tool is not installed and showing the install command.

## Translate dragged text

Drag selected text instead of files and tap the translate key (<kbd>⌘</kbd> by default) to open a fourth, purple wheel with your languages; release over one to translate. The translation replaces the text where the app allows it, in text fields and in Safari or Chromium pages, and opens in a window otherwise, with a neon glow on the text while the AI answers. See [Translate selected text](/docs/launcher/translate/) for the options, the engine and model, the browser setting and the limits.

## Tools per format

A table of 26 tools sets what the Tools wheel and the Convert view offer for each file type. Tools without a window use presets or typed commands. See [File tools](/docs/files/tools/compress/) and [Typed commands](/docs/files/typed-commands/).

## Convert Finder Selection

A command (give it a hotkey in Settings) that reads what is selected in Finder and opens the conversion wheel for it. It does not intercept Return, so Finder's rename keeps working unless you turn on the setting below. The Finder read needs Automation permission.

## Enter on Finder selection

An opt-in setting in Settings > General, row “Enter on Finder selection”: press Return on selected files in Finder to open the Convert wheel instead of renaming. It needs Accessibility access and is off by default. The Convert Finder Selection command also works.

## Drop fix

Files dragged from Finder arrive as file reference URLs (`file:///.file/id=…`); they are now resolved to the real path. Before, a dragged PNG looked like a file without an extension.

:::caution[Limits]
It is not animated yet. Plain <kbd>↵</kbd> on a file result still opens it.
:::

## Related

- [Conversions](/docs/files/conversions/)
- [File actions](/docs/files/file-actions/)
- [Settings](/docs/launcher/settings/)
