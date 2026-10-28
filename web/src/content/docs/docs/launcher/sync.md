---
title: "Sync with iCloud"
description: "Keep Ferrocast's settings, snippets, quicklinks, notes, clipboard, chats and custom themes in step between your Macs with iCloud Drive."
sidebar:
  order: 20
---
Sync keeps Ferrocast's data the same on every Mac you use. It goes through the iCloud Drive folder, so you need iCloud Drive turned on in System Settings. It is **off by default**: nothing is read from or written to iCloud Drive until you turn it on.

Turn it on in Settings → Advanced → Sync with iCloud. The status line under **Sync now** shows the time of the last sync and how many other Macs Ferrocast has seen, or the reason it cannot run (for example, the iCloud Drive folder is missing). **Sync now** runs a sync right away.

## What syncs

- **Settings**, hotkeys included.
- **Snippets** and **quicklinks**.
- **Custom themes**. Each theme is its own item, so a theme you edit, rename or delete reaches your other Macs. If you rename the selected theme on one Mac, the other Macs keep it selected. Two themes with the same name made on different Macs both stay; rename one of them.
- **Notes**, including pinned notes and Recently Deleted.
- **Clipboard history**, with the images of image entries. Only what Ferrocast recorded on a Mac is shared. Entries from password managers are never recorded, so they are never shared. Your history retention setting still applies.
- **Ask AI chats**. Ferrocast now saves each conversation so it can reach your other Macs.

## What stays on each Mac

- Script folders, the Obsidian vault, Launch at login, the camera and microphone you picked for recording, the screenshot and recordings folders, update preferences and state, the automatic tiling switch and the layout of each display (the other tiling settings do sync), and window and note state such as the last open note and the Notes zoom.
- The Raycast extensions switches: Run Raycast extensions and the per-extension on/off switches. Extension preferences and data also stay on each Mac.
- The Sync switch itself: you turn it on separately on each Mac.
- **macOS permissions** (Accessibility, Screen Recording, Input Monitoring and so on). They belong to each Mac and Ferrocast cannot copy them. Grant them again on every Mac.
- **API keys** in the Keychain. Ferrocast is signed ad hoc, which does not allow iCloud Keychain, so enter each key on every Mac.

## Conflicts

Ferrocast merges item by item, and the newest edit wins.

- Every snippet, quicklink, custom theme, note, clipboard entry and chat is its own item.
- **Every setting is its own item** (and every nested setting, such as one capture option or one command's options). If one Mac changes the theme and another changes the hotkey, both changes are kept.
- An item added on one Mac is never lost because another Mac did not have it.
- A deletion reaches your other Macs. If another Mac edits the item after the deletion, the item comes back.
- A deletion is remembered for good, so an old Mac or a leftover folder in iCloud Drive cannot bring a deleted item back.
- Clipboard deletions are remembered for 400 days (the 3,000 newest), longer than the longest history setting. A pinned clipboard entry older than 400 days is not synced, so edits to it stay on the Mac where you made them.
- Each Mac writes only its own files in the `Ferrocast` folder of iCloud Drive (`devices/<id>/`), so iCloud has no conflicting copies to create. A file Ferrocast cannot read is skipped.

## Turning sync on for the first time

- What a Mac already has counts as the oldest version, even if iCloud has not shown the other Macs' files yet. A Mac that joins later takes on the newer version of each item your other Macs have, and anything only it has is added.
- An item you never changed (a setting, snippet or quicklink exactly as it came with Ferrocast) counts as unchanged. A version you changed on any Mac beats it whatever the times, so a new Mac that synced first cannot wipe the customisations of your old one. Changing a value back to its default on purpose is an edit like any other.
- A built-in snippet or quicklink you deleted before turning sync on stays deleted when you add a Mac.
- The Mac that synced first keeps its data. After its second sync (about 30 seconds) its items count as made at its first sync, which beats every Mac that joins later. Items a Mac only took over from another Mac do not get that time, so an edit made elsewhere in between still wins.
- If two Macs turn sync on for the first time before either has synced twice, with different versions of the same item, the one with the greater device id wins on both, so they still end up the same.
- Ferrocast keeps a copy of this Mac's settings as `settings.before-sync.json` in the data folder before the first merge changes them.
- If the sync state of a Mac is lost, Ferrocast rebuilds it from that Mac's own file in iCloud Drive, so edits and deletions made since its last sync are kept.

## When it syncs

- Every 30 seconds, when you open the launcher, and when you press **Sync now**.
- While you are typing in the Notes window, merged notes wait until you pause for two seconds.
- Turning sync off stops all reading and writing but leaves the folder in iCloud Drive as it is.

:::note[Clipboard history]
The history now leaves the Mac when sync is on, and it is not encrypted in iCloud Drive beyond what iCloud itself provides. Leave sync off if you do not want that.
:::
