---
title: "Data and privacy"
description: "Where Ferrocast keeps its data, what is stored unencrypted, which items live in the Keychain, and what leaves your Mac: nothing unless you ask it to."
sidebar:
  order: 1
---
Ferrocast keeps its data in JSON files in `~/Library/Application Support/Ferrocast`. The files are unencrypted and open in any editor. Set `FERROCAST_DATA_DIR` to use another folder (see [Environment variables](/docs/reference/environment-variables/)).

## What lives where

- **JSON files** in the data folder: settings, clipboard history, snippets, quicklinks, notes and the File Shelf list.
- **Keychain**: AI keys (an Anthropic key, an OpenAI-compatible server key) and the ChatGPT sign-in session, which is refreshed automatically.
- **Log**: an error log in the data folder; open it with the **Reveal Log File** command.
- **Backups**: export leaves the clipboard history out.

## What leaves your Mac

- File conversions and file tools never upload anything; they run locally with macOS tools first and optional ffmpeg, LibreOffice, cwebp, Ghostscript and unrar when installed.
- The clipboard history never leaves your Mac, and entries copied from a password manager such as 1Password are skipped on purpose.
- Currency rates come from frankfurter.dev (ECB data, cached 12 hours).
- Ask AI talks to Ollama on your own Mac by default. It only contacts the cloud if you point it at LM Studio, OpenAI, the Anthropic API, Sign in with ChatGPT or Claude Code.
- Script Command arguments are passed on the command line and can be seen with `ps` while the script runs.

:::caution[Limits]
The JSON files are not encrypted. Deleted clipboard images are removed from disk, and failed saves are now reported.
:::

## Related

- [Clipboard history](/docs/launcher/clipboard/)
- [Ask AI](/docs/launcher/ask-ai/)
- [Settings](/docs/launcher/settings/)
