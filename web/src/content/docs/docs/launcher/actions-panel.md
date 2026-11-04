---
title: "Action panel (⌘K)"
description: "Press Command K on a result in Ferrocast to open the action panel with the actions and shortcuts for the selected item, such as favorites."
sidebar:
  order: 4
---
The action panel lists what you can do with the selected item: aliases, a hotkey for any command, favorites and more. For file results it also holds the file actions, for example Compress to ZIP, and Quick Look.

Select a result and press <kbd>⌘</kbd> <kbd>K</kbd>. In the Convert view, <kbd>⌘</kbd> <kbd>K</kbd> on an editor row shows Open Editor.

## Applications

An app result offers Open Application (<kbd>↵</kbd>), Show in Finder, Copy Path, and **Move to Trash** (<kbd>⌘</kbd> <kbd>⌫</kbd>). Right-click an app row to get the same list. Move to Trash always asks first and names the app. It moves only the `.app` bundle to the Trash, so Finder's **Put Back** restores it; nothing in `~/Library` is touched and nothing is deleted permanently. The app disappears from the results at once.

Ferrocast refuses, with a message and no confirmation, for apps in `/System`, Apple apps that macOS protects, Ferrocast itself, and anything that is not an `.app` bundle. If the app is running, quit it first: Ferrocast never closes it for you. If macOS asks for permission it cannot get, the system's own error is shown.

## Related shortcuts

- <kbd>↵</kbd> runs the primary action and <kbd>⌘</kbd> <kbd>↵</kbd> the secondary action; <kbd>⌘</kbd> <kbd>↵</kbd> never fires “Add to Favorites” by position.
- <kbd>⌘</kbd> <kbd>Y</kbd> Quick Look the selected file.
- <kbd>⌃</kbd> <kbd>X</kbd> deletes the selected entry.

The full list is in [Keyboard shortcuts](/docs/launcher/keyboard-shortcuts/).

## Related

- [Root search](/docs/launcher/search/)
- [File actions](/docs/files/file-actions/)
