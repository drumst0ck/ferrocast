---
title: "Run Raycast extensions"
description: "Run the extensions you installed with Raycast inside Ferrocast. Off by default; needs Node.js 22 or newer. What works, what does not, and the risks."
sidebar:
  order: 16
---
Ferrocast runs Raycast extensions: the ones you already installed with Raycast show up in the launcher next to Ferrocast's own commands. Ferrocast reads them from `~/.config/raycast/extensions` and runs their code with its own implementation of the Raycast extension API. It never contacts Raycast's Store, and it does not install anything for you.

Raycast is a trademark of its owner. Ferrocast is not affiliated with, endorsed by or sponsored by Raycast.

## Turn it on

This is **off by default**. Until you turn it on, Ferrocast does not read the extensions folder and starts no extension code.

1. Open Settings → Integrations and find the **Raycast extensions** group.
2. Switch on **Run Raycast extensions**.
3. Check the **Node.js 22 or newer** row. If it says **Ready**, you are done.
4. Each extension found in the folder is listed with its own switch. An extension you switch off has no commands in the launcher and cannot be started.

The commands appear in the launcher by their own names, with the extension's name as the subtitle. If you have no extensions in the folder yet, the list says so; install some with Raycast first.

These switches are not synced: Node.js and the installed extensions exist per Mac, so [Sync with iCloud](/docs/launcher/sync/) leaves them out.

## Node.js

Extensions are JavaScript, so Ferrocast needs Node.js 22 or newer. It looks for one on your Mac first (for example from Homebrew). If it finds none, **Download Node.js (about 26 MB)** fetches the official build from nodejs.org. It happens only when you press the button. Ferrocast checks the file against a SHA-256 checksum that is built into the app, refuses anything that does not match, and keeps only the `node` program in its own data folder, under `~/Library/Application Support/Ferrocast/runtime`. Nothing is installed system-wide. The app itself does not contain Node.js.

## Preferences

Many extensions have preferences (an API key, a default folder). Press **Preferences…** on the extension's row to edit them in a form. Text fields, passwords, checkboxes and dropdowns are supported; file, folder and app preferences are typed as a path or a name. Passwords are masked while you type and are never written to Ferrocast's log.

Values are saved to `extensions/<name>/preferences.json` in Ferrocast's data folder, as plain text in that file (not in the Keychain). If you start a command that needs a required preference you have not set, Ferrocast opens this form instead of running it.

## What works

Extensions are drawn with Ferrocast's own interface, not a web view. These parts of the API work:

- **Lists** with sections, subtitles, accessories, search (including searching as you type), a detail pane, and dropdown filters, which appear as options in the actions panel (⌘K).
- **Detail** screens with Markdown.
- **Forms** with text, password, text area, checkbox, dropdown, date (typed as `YYYY-MM-DD`) and tags. ⌘↵ submits.
- **Actions** and their shortcuts, pushing and popping screens, toasts, copying and pasting, opening URLs and files, showing a file in Finder.
- **Storage and data**: LocalStorage, Cache, the preferences, the list of your apps, the frontmost app, the selected text and confirmation dialogs.
- **Grids** are shown as lists, with the cell content as the icon.

To measure this, we ran 20 popular extensions from Raycast's public repository (github.com/raycast/extensions, MIT licence) that need no account, API key or AI. We took the 20 with the highest install counts shown on their Store pages, out of about 60 candidates that need no account, key or AI. They are not the whole Store. They have 40 commands that open a view, and we drove each one without the interface: launch it, search, open the first action, push a screen and go back, submit a form with its default values. 34 of the 40 worked, 4 only waited for input we did not give (for example an empty clipboard), 1 failed because Homebrew itself refused to load a cask from an untrusted tap, and 1 showed an empty history. The test does not prove that every command of every extension works, and it did not use the app's window. Your extensions may behave differently.

## What does not work yet

- **Menu bar commands**: they are not listed.
- **File picker fields** in forms and the link accessory.
- **Sign-in with OAuth** and the Raycast AI features.
- **Some actions**: move to Trash, open with, create a snippet or quicklink, and pick a date.
- **Launching a command of another extension**, and anything that needs Raycast's own account, sync or Store.
- **Detail pane Markdown in lists** is shown as plain text.

When an extension uses something Ferrocast cannot draw, the screen says "Not supported in Ferrocast yet: ", the name of the part, the extension and a link to this page. The rest of the extension keeps working where it can.

## Security and privacy

**Extensions are programs written by other people. They run on your Mac with your permissions.** An extension can read your files, use the network, see what you copy, read the text you have selected in other apps and see the list of apps you use. Turn this on only for extensions you installed and trust.

What Ferrocast does to limit the damage:

- Each command runs in its own Node.js process, which is stopped when you leave the command. Calls beyond 8 in flight and `launchCommand` beyond 5 per 10 seconds are refused with an error, and stderr is cut after 500 lines. A command is stopped when it writes a stdout line over 16 MB, when its input queue is full, when it draws nothing for 30 seconds, when a command without a screen runs for 60 seconds, or when it has been hidden for 90 seconds.
- The process does not get your environment variables, only a short list that programs need to run.
- Ferrocast's own open and Show in Finder calls are checked. An extension still runs with your full user rights and can do more by itself.
- A command with a screen only gets the selected text and dialogs while that screen is visible; a command without a screen keeps them while it is hidden.
- What an extension writes to stderr can appear in Ferrocast's log. Ferrocast logs the names of saved preferences, never their values.
- A process that detaches itself (for example with `setsid`) can outlive the stop.

None of this makes an untrusted extension safe. Ferrocast does not send your extensions, preferences or data anywhere; what an extension itself sends over the network is up to its code.
