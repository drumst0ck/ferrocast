---
title: "Custom themes"
description: "Make your own Ferrocast colours in Settings: accent, background, surface and text, with a live preview, contrast warnings and export or import as a file."
sidebar:
  order: 13
---
Besides System, Dark and Light, Ferrocast can use colours you choose. A custom theme is a saved set of colours on a light or dark base. Everything else (hover, selected rows, borders, readable accent text) is worked out from them.

## Make a theme

1. Open Settings → General (<kbd>⌘</kbd> <kbd>,</kbd>).
2. In **Custom themes**, click **New from current**. Ferrocast copies the look in use (dark or light) into a theme called "Dark copy" or "Light copy" and selects it.
3. Change the colours. Each one has a colour picker and a hex field (`#RRGGBB`; `#RGB` also works). The launcher, Settings and the tool windows repaint as you edit, and a small launcher under the editor shows the result.

The editor has these values:

| Value | Used for |
| --- | --- |
| Base | Light or dark. Sets how hover and selected colours are derived and whether the Notes window is light or dark. |
| Accent | Buttons, the caret, highlights and the selected switch. |
| Background | The window. |
| Surface | Cards, fields and controls. |
| Text | Titles and results. |
| Secondary text | Descriptions and hints. |
| Separator | Lines between rows. |

System, Dark and Light themes are built in and cannot be edited. **New from current** copies them into a theme you can edit. With a custom theme selected the button reads **Duplicate** and copies that theme.

## Pick, rename, delete

Use the drop-down in **Custom themes** to switch theme. Choose **None** or click System, Dark or Light in **Appearance** to go back to a built-in look. **Rename** changes the name (names are unique, ignoring case). **Delete** removes the selected theme.

## Contrast warnings

Text has to stay readable. When text on the background, text on the surface, or secondary text on the background falls below a 4.5:1 contrast ratio (WCAG AA for normal text), a warning under the editor names the pair and its ratio. It is only a warning: you can keep the theme.

## Share a theme

**Export…** saves the selected theme as a `.ferrocast-theme` file (JSON with a format name and version). **Import…** reads one; if the name is taken, the imported theme gets a number ("Ocean 2"). Files from a newer format version, files that are not themes and files with an invalid colour are refused with a message.

Themes are stored in `themes.json` in the data folder; the selected one is saved in `settings.json`.

:::caution[Limits]
- Colours that carry meaning stay fixed: the green, amber and red status chips, the video preview surface in the recording editor, and the welcome tour slides.
- The Notes window follows the theme's base (light or dark), not its individual colours.
- Settings → General has no searchable row for each colour; search for "Custom themes" instead.
:::

## Related

- [Settings and backup](/docs/launcher/settings/)
- [Notes](/docs/launcher/notes/)
