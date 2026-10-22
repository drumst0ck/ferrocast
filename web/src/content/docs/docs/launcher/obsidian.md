---
title: "Obsidian: search your vault"
description: "Search the notes in your Obsidian vault from the launcher, preview them beside the list, open them in Obsidian and copy wikilinks. Files are read locally."
sidebar:
  order: 15
---
Search Obsidian finds the notes in your vault from the launcher, shows a preview next to the list and opens the note in Obsidian. Ferrocast reads the Markdown files on your Mac directly, so it also works while Obsidian is closed. Only the actions that open Obsidian need the app.

## Choose your vault

1. Open Settings → Integrations and find the **Obsidian** group.
2. Pick a vault in **Vault**. Ferrocast lists the vaults Obsidian knows about by name. If yours is not there, use **Choose Folder…** under **Vault folder** and select the vault's folder.
3. The row **Obsidian app** tells you whether Obsidian was found on this Mac.

Until a vault is chosen, Search Obsidian shows **Choose your Obsidian vault** and one row, **Open Settings → Integrations**, which takes you to that group.

## Search

Open **Search Obsidian** from the launcher. You can give the command an alias or a hotkey like any other command.

- With an empty search box the list shows the 30 notes you changed most recently, under **Recent**.
- Typing matches the note title, its `aliases` from the frontmatter and its path. A note's title is the frontmatter `title`, else its first `# ` heading, else the file name. Matches are listed under **Notes**, best match first, up to 30.
- With 3 or more characters, Ferrocast also looks inside the notes. It waits a quarter of a second after your last key, then lists the notes that contain your text and did not match by title under **In notes**.

Each row shows the title, the folder it lives in (empty for notes at the root of the vault) and when it was last modified: "2h ago", "yesterday", or the date.

The first time you open the command in a session, Ferrocast indexes the vault in the background and the list says **Indexing your vault…** until it is ready. The next time you open it, the last index appears at once and refreshes behind the scenes, so notes you added since then show up a moment later.

## Preview

The selected note appears in a pane on the right: its title and path, then its content with headings, bullets, numbered lists, task checkboxes, quotes, code blocks and rules. Frontmatter is hidden and `[[wikilinks]]` show as plain text. The preview reads the first 200 KB of the note. Press <kbd>⌘</kbd> <kbd>Y</kbd> to hide or show the pane; when it is hidden the list uses the full width and nothing is read.

If a note was moved or deleted after the index was built, or cannot be read, the pane says **Could not read this note.**

## Actions

Press <kbd>↵</kbd> to open the selected note in Obsidian, or <kbd>⌘</kbd> <kbd>K</kbd> for all actions.

| Action | Shortcut | What it does |
|---|---|---|
| **Open in Obsidian** | <kbd>↵</kbd> | Opens the note in your vault through an `obsidian://open` link. |
| **Reveal in Finder** | <kbd>⌘</kbd> <kbd>↵</kbd> | Shows the file in Finder. |
| **Copy Wikilink** | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> | Copies `[[Note name]]`. Obsidian resolves links by file name, so when the title differs from the file name you get `[[File name\|Title]]` and the link still works. |
| **Copy Path** | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> | Copies the full path of the file. |
| **Quick Look** | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>Y</kbd> | Shows the file in the system Quick Look panel. <kbd>⌘</kbd> <kbd>Y</kbd> alone toggles the preview pane. |
| **Search in Obsidian** | | Runs the text in the search box in Obsidian's own search (`obsidian://search`). |

Links longer than 8,000 characters are refused with a toast instead of being cut. If Ferrocast has found that Obsidian is not installed, **Open in Obsidian** and **Search in Obsidian** show "Obsidian is not installed. Download it from obsidian.md." and open nothing. Until that check has finished, Ferrocast tries to open the link anyway and lets macOS answer.

## What is read

- Ferrocast reads your vault on your Mac. Nothing is uploaded and no network request is made.
- The index holds each note's path, title, aliases and modified time. It lives in memory and is rebuilt each time you open the command.
- Searching inside notes reads the files from disk, with a cap on how much it reads per search.
- Nothing in your vault is changed, moved or deleted.
- If your vault is in a cloud-synced folder (iCloud Drive, Dropbox, OneDrive), the first indexing in a session can take several seconds while the provider answers. The window stays responsive. Notes the provider has not downloaded yet are left out of the search inside notes, so a search never starts mass downloads. They are still found by title. Selecting such a note shows its preview, which makes the provider download that one file.

## Troubleshooting

| Message or symptom | What to do |
|---|---|
| **The vault folder was not found** | The folder was moved, renamed or sits on a drive that is not connected. Choose the vault again in Settings → Integrations. |
| **Choose your Obsidian vault** | No vault is saved yet. Open Settings → Integrations and pick one. |
| **Obsidian is not installed. Download it from obsidian.md.** | Install Obsidian. Search, preview, Reveal in Finder and the copy actions keep working without it. |
| A note does not show up | Folders that start with a dot (`.obsidian`, `.trash`, `.git`) are skipped, only `.md` files are indexed, files over 2 MB are skipped, and a vault is indexed up to 50,000 notes. |
| A word inside a note is not found | Searching inside notes starts at 3 characters, skips notes that are still cloud placeholders and stops after reading a fixed amount of text. Search by title, or use **Search in Obsidian**. |
| **That link is too long for Obsidian (over 8,000 characters).** | Shorten the text you are searching for. |
| **Could not read this note.** | The file was deleted, moved or is not readable. Reopen the command to refresh the index. |
