---
title: "Frequently asked questions"
description: "Answers about Ferrocast: price, install, permissions, privacy, Ask AI, ChatGPT and Claude plans, Raycast, file conversion and what is missing now."
sidebar:
  order: 4
---
## General

### How do I open it?
Press <kbd>⌥</kbd> <kbd>Space</kbd> from any app. You can change the shortcut in Settings.

### Is it free?
Completely. MIT licence, no account, no Pro tier waiting behind a paywall.

### Will it run on my Mac?
It runs on Apple Silicon Macs. Intel Macs are not supported.

### Do I need to be a developer to install it?
No, but you do need to open Terminal once and paste four lines. Homebrew does the rest, and it compiles for a few minutes. See [Install](/docs/getting-started/install/).

### Why does it ask for Accessibility access?
Because moving windows, pasting for you and expanding snippets all need it. Search, the calculator and AI work without it. A welcome screen walks you through it.

### Is my clipboard history private?
It never leaves your Mac. It sits in a JSON file in Application Support, and anything copied from a password manager like 1Password is skipped on purpose.

### What is missing today?
Honestly: an extension store and sync between Macs. If you live inside Raycast extensions, Ferrocast is not a drop-in replacement yet.

## AI and migration

### Does Ask AI send my text to the cloud?
Only if you tell it to. Out of the box it talks to Ollama on your own Mac. Point it at LM Studio, OpenAI or the Anthropic API if you prefer; keys go in the Keychain.

### Can I use my ChatGPT or Claude subscription?
Yes, with caveats spelled out in the app. Claude: if Claude Code is installed and signed in, Ferrocast uses it as a provider, since it is Anthropic's own tool. ChatGPT: Ferrocast can sign in with your account the way OpenAI's Codex CLI does; it works, but OpenAI has not blessed third-party apps doing that, so the API with a key is the by-the-book route. See [Ask AI](/docs/launcher/ask-ai/).

### Coming from Raycast. Will my stuff work?
Your Script Commands run as they are, and snippets import from a JSON export. Quicklinks you recreate in a minute. See [Migrating from Raycast](/docs/getting-started/migrating-from-raycast/).

## File conversion

### Are conversions really local?
Yes. Nothing is uploaded. Conversions run on your Mac and use the tools that ship with macOS first. ffmpeg, LibreOffice, cwebp, unrar and Ghostscript are optional and are used only when they are installed; the [conversion reference](/docs/files/conversions/) says which pairs need one.

### What can it convert?
There are 46 input formats and 275 conversion pairs across images, audio, video, PDF, documents, slides, spreadsheets, subtitles and archives.

### Does it overwrite my files?
No. The result is written next to the original with the same name and the new extension. If that name is taken it adds 2, 3 and so on.

### Can I press Enter on a file in Finder?
Yes, as an opt-in setting. In Settings > General, turn on “Enter on Finder selection” (off by default, needs Accessibility) and pressing Return on selected files in Finder opens the Convert wheel instead of renaming. The Convert Finder Selection command also works, and Option-Enter on a file result in the launcher does the same.

## Related

- [Troubleshooting](/docs/reference/troubleshooting/)
