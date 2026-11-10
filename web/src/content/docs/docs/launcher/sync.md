---
title: "Sync with iCloud"
description: "Keep Ferrocast's settings, snippets, quicklinks, notes and their images, clipboard, chats and themes in step between Macs with iCloud Drive."
sidebar:
  order: 20
---
Sync keeps Ferrocast's data the same on every Mac you use. It goes through the iCloud Drive folder, so you need iCloud Drive turned on in System Settings. It is **off by default**: nothing is read from or written to iCloud Drive until you turn it on.

Turn it on in Settings → Advanced → Sync with iCloud. The status line under **Sync now** shows the time of the last sync and how many other Macs Ferrocast has seen, or the reason it cannot run (for example, the iCloud Drive folder is missing). **Sync now** runs a sync right away.

## What syncs

- **Settings**, hotkeys included.
- **Snippets** and **quicklinks**.
- **Custom themes**. Each theme is its own item, so a theme you edit, rename or delete reaches your other Macs. If you rename the selected theme on one Mac, the other Macs keep it selected. Two themes with the same name made on different Macs both stay; rename one of them.
- **Notes**, including pinned notes and Recently Deleted, and the images in them (see below).
- **Clipboard history**, with the images of image entries. Only what Ferrocast recorded on a Mac is shared. Entries from password managers are never recorded, so they are never shared. Your history retention setting still applies.
- **Ask AI chats**. Ferrocast now saves each conversation so it can reach your other Macs.

## What stays on each Mac

- Script folders, the Obsidian vault, Launch at login, the camera and microphone you picked for recording, the screenshot and recordings folders, update preferences and state, the automatic tiling switch and the layout of each display (the other tiling settings do sync), and window and note state such as the last open note and the Notes zoom.
- The Raycast extensions switches: Run Raycast extensions and the per-extension on/off switches. Extension preferences and data also stay on each Mac.
- The Sync switch itself: you turn it on separately on each Mac.
- **macOS permissions** (Accessibility, Screen Recording, Input Monitoring and so on). They belong to each Mac and Ferrocast cannot copy them. Grant them again on every Mac.
- **API keys** in the Keychain. Ferrocast is signed ad hoc, which does not allow iCloud Keychain, so enter each key on every Mac.

## Note images

Images in notes travel as separate files, next to the note records, in the same `Ferrocast` folder of iCloud Drive.

- Each Mac writes only its own folder, `devices/<id>/attachments/`, so two Macs never write the same file. An image is uploaded once, by the Mac that has it, and only when a note links it. A Mac that downloaded an image does not upload it again.
- When a synced note links an image this Mac does not have, Ferrocast copies it from the other Mac's folder, after checking that the file is complete and is the one the note names. Until it arrives (iCloud may still be downloading it) the note shows a placeholder, the sync status counts the file, and the picture appears when it is here.
- An image is never removed from iCloud Drive while any Mac's notes link it, and a Mac never removes one while another Mac's notes cannot be read. An image that nothing links is removed 7 days after the last link went away.
- Each image is at most 20 MB. Turning sync off stops all copying; nothing is uploaded or removed.

## Conflicts

Ferrocast merges item by item, and the newest edit wins. If two Macs edit the same note, the older text is not dropped: it is kept as a new note ending in "(conflict copy from <Mac name>)", and Ferrocast shows a toast. Edits made after receiving another Mac's version win even if this Mac's clock is behind.

- Every snippet, quicklink, custom theme, note, clipboard entry and chat is its own item.
- **Every setting is its own item** (and every nested setting, such as one capture option or one command's options). If one Mac changes the theme and another changes the hotkey, both changes are kept.
- An item added on one Mac is never lost because another Mac did not have it.
- A deletion reaches your other Macs. If another Mac edits the item after the deletion, the item comes back.
- A deletion is remembered until every Mac that has not been forgotten has confirmed it received it (see below), so an old Mac, a Mac that comes back after months or a leftover folder in iCloud Drive cannot bring a deleted item back.
- Clipboard deletions are remembered for 400 days (the 3,000 newest), longer than the longest history setting. A pinned clipboard entry older than 400 days is not synced, so edits to it stay on the Mac where you made them.
- Each Mac writes only its own files in the `Ferrocast` folder of iCloud Drive (`devices/<id>/`), so iCloud has no conflicting copies to create. A file Ferrocast cannot read is skipped.
- An item written by a newer Ferrocast that this Mac's version cannot read is left alone: this Mac does not show it and never publishes it as deleted, so your other Macs keep it as the Mac that wrote it published it. It appears once this Mac is updated.

## Clipboard documents

Each Mac's clipboard document holds only the entries copied on that Mac, so the size in iCloud Drive does not grow with the number of Macs. Every Mac still ends up with the whole history, because every Mac reads every other Mac's document.

- An entry you copied on one Mac is published by that Mac. A Mac that only took it over does not publish it again, unless another Mac does not already carry it (for instance, the Mac that copied it was forgotten): then the Mac with the lowest id keeps it available.
- If you edit an entry from another Mac (pin it, for example), the Mac that edited it publishes it. If you delete it, that Mac publishes the deletion, which reaches every Mac.
- Ferrocast v0.7.0 and older merge every document, so they keep working with the new ones. Their documents still hold the entries they took over; these shrink to the Mac's own entries on the first sync with this version.
- If a Mac loses its clipboard history, it gets it back from its own document and the other Macs' documents.
- A Mac only uploads the images of the entries it publishes.

## Macs and deletions

Settings → Advanced → Sync lists the Macs that have published sync data: the name each one published and when it was last seen. **Forget this Mac**, behind a confirmation, removes that Mac's folder (`devices/<id>/`: its documents and images) from the sync folder, and this Mac stops reading it. It is not offered for the Mac you are using, and Ferrocast never removes this Mac's folder or another Mac's. If the forgotten Mac syncs again, it publishes again and appears again in the list. Use it for a Mac you retired or reinstalled, so it stops taking space.

Deletions are cleaned up only when that is safe. Each Mac's sync document records when it was written (by that Mac's own clock) and, for every other Mac, which of that Mac's documents it has read and applied. A Mac removes a deletion it published only when every Mac that has not been forgotten has confirmed a document that already contained it. Until then the deletion stays, so a Mac that comes back after weeks or months still learns about everything you deleted meanwhile and removes those items; nothing is purged on the returning Mac, and notes it never uploaded are kept. Clocks that run fast or slow do not matter, because only each Mac's own clock is compared with itself.

A Mac that stays off, runs an older Ferrocast or has never confirmed a document blocks the cleanup of deletions, which then stay in the documents (they are small). **An inactive Mac must be forgotten for cleanup to proceed.** Forgetting removes that Mac's own sync data and records in your other Macs' documents that it was forgotten. Built-in snippet and quicklink deletions are never cleaned up.

When a deletion is cleaned up, the Macs keep a compact list of the deleted items' ids and times, and every Mac passes the list on. A record of that item that is not newer than the deletion is ignored, so an old copy cannot bring the item back; an edit made after the deletion still counts as an edit and wins. A Mac that comes back after it was forgotten, a Mac restored from an older backup and a Mac that lost its sync state start over: they read the other Macs first, learn what was cleaned up, drop their old copies of those items and do not publish them. Notes and other items that Mac never uploaded are not on the list and are kept. A Mac on Ferrocast 0.8.0 or older does not confirm anything, so it blocks cleanup until it is updated or forgotten; if a forgotten Mac still on 0.8.0 comes back, it keeps its stale copies on that Mac only (the newer Macs ignore them), and it blocks cleanup again until it is updated.

If a forgotten Mac, or one that has been away for a long time, had published a clipboard deletion, that deletion goes with its folder. Clipboard deletions also expire on their own after 400 days, so a third Mac that was also away may bring such an entry back for itself.

## Turning sync on for the first time

- What a Mac already has counts as the oldest version, even if iCloud has not shown the other Macs' files yet. A Mac that joins later takes on the newer version of each item your other Macs have, and anything only it has is added.
- An item you never changed (a setting, snippet or quicklink exactly as it came with Ferrocast) counts as unchanged. A version you changed on any Mac beats it whatever the times, so a new Mac that synced first cannot wipe the customisations of your old one. Changing a value back to its default on purpose is an edit like any other.
- A built-in snippet or quicklink you deleted before turning sync on stays deleted when you add a Mac.
- The Mac that synced first keeps its data. After its second sync (about 30 seconds) its items count as made at its first sync, which beats every Mac that joins later. Items a Mac only took over from another Mac do not get that time, so an edit made elsewhere in between still wins.
- If two Macs turn sync on for the first time before either has synced twice, with different versions of the same item, the one with the greater device id wins on both, so they still end up the same.
- Ferrocast keeps a copy of this Mac's settings as `settings.before-sync.json` in the data folder before the first merge changes them.
- If the sync state of a Mac is lost, Ferrocast rebuilds it from that Mac's own file in iCloud Drive, so edits and deletions made since its last sync are kept.
- If a data file on one Mac cannot be read (Ferrocast keeps it next to the original as `.corrupt`), that Mac starts over from its own file in iCloud Drive and what your other Macs have, instead of telling them everything was deleted. The same happens when Ferrocast cannot read the file at all. The status line says so.

## When it syncs

- Every 30 seconds, when you open the launcher, and when you press **Sync now**.
- While you are typing in the Notes window, merged notes wait until you pause for two seconds.
- Turning sync off stops all reading and writing but leaves the folder in iCloud Drive as it is.

:::note[Clipboard history]
The history now leaves the Mac when sync is on, and it is not encrypted in iCloud Drive beyond what iCloud itself provides. Leave sync off if you do not want that.
:::
