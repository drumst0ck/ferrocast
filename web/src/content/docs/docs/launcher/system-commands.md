---
title: "System commands, timers and notes"
description: "Ferrocast system commands: Wi-Fi, Keep Awake, Kill Process, timers and pomodoro, notes, emoji, Quick Look and Check for Updates."
sidebar:
  order: 10
---
System commands cover everyday Mac tasks from the launcher: Toggle Wi-Fi, Toggle Keep Awake (caffeinate), Kill Process, timers, notes, emoji, Quick Look for files and Check for Updates.

Run them from the root search by name.

## Commands

- Kill Process: processes listed busiest first with app icon, CPU and memory; <kbd>↵</kbd> terminates (SIGTERM) and <kbd>⌘</kbd> <kbd>↵</kbd> force-kills (SIGKILL).
- Timers: `timer 10m`, `pomodoro`, `set a timer for 1h 30m`; a notification and a sound when done; the Timers view lets you cancel.
- Notes and Emoji (colour emoji rendered with AppKit; recently used emoji are kept).
- Quick Look on a file with <kbd>⌘</kbd> <kbd>Y</kbd>; Check for Updates asks GitHub for a newer stable release (Homebrew installs run `brew upgrade`).
- Reveal Log File opens the error log. Ferrocast is a single instance.
- Every builtin and system command uses an SF Symbol rendered by AppKit.

## Related

- [Root search](/docs/launcher/search/)
- [Troubleshooting](/docs/reference/troubleshooting/)
