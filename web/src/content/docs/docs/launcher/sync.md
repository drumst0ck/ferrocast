---
title: "Sync with iCloud"
description: "Keep Ferrocast's settings, snippets, quicklinks, notes, clipboard history and Ask AI chats in step between your Macs through iCloud Drive. Off by default."
sidebar:
  order: 20
---
Sync keeps Ferrocast's data the same on every Mac you use. It goes through the iCloud Drive folder, so you need iCloud Drive turned on in System Settings. It is **off by default**: nothing is read from or written to iCloud Drive until you turn it on.

Turn it on in Settings → Advanced → Sync with iCloud. The status line under **Sync now** shows the time of the last sync and how many other Macs Ferrocast has seen, or the reason it cannot run (for example, the iCloud Drive folder is missing). **Sync now** runs a sync right away.

## What syncs

- **Settings**, hotkeys included.
- **Snippets** and **quicklinks**.
- **Notes**, including pinned notes and Recently Deleted.
- **Clipboard history**, with the images of image entries. Only what Ferrocast recorded on a Mac is shared. Entries from password managers are never recorded, so they are never shared. Your history retention setting still applies.
- **Ask AI chats**. Ferrocast now saves each conversation so it can reach your other Macs.

## What stays on each Mac

- Script folders, the Obsidian vault, Launch at login, the camera and microphone you picked for recording, the screenshot and recordings folders, update preferences and state, and window and note state such as the last open note and the Notes zoom.
- The Sync switch itself: you turn it on separately on each Mac.
- **macOS permissions** (Accessibility, Screen Recording, Input Monitoring and so on). They belong to each Mac and Ferrocast cannot copy them. Grant them again on every Mac.
- **API keys** in the Keychain. Ferrocast is signed ad hoc, which does not allow iCloud Keychain, so enter each key on every Mac.

## Conflicts

Ferrocast merges item by item. Each snippet, quicklink, note, clipboard entry and chat is compared on its own, and the newest edit wins. An item added on one Mac is never lost because another Mac did not have it. A deletion reaches your other Macs too; if another Mac edits the item later than the deletion, the item comes back. The settings are one item: the most recent change to them on any Mac wins.

Each Mac writes only its own files in the `Ferrocast` folder of iCloud Drive (`devices/<id>/`), so iCloud has no conflicting copies to create. A file another Mac wrote that Ferrocast cannot read is skipped.

The first time you turn sync on in a Mac, what it already has counts as the oldest version of each item, even if iCloud has not shown the other Macs' files yet. A Mac that joins later therefore takes on the settings and the newer version of each item that your other Macs have, and anything only it has is added to the rest. If a Mac still has a setting or a built-in snippet or quicklink exactly as it came with Ferrocast and never changed it, that version counts as unchanged: a version you customised on another Mac beats it whatever the times, so adding a new Mac that synced first cannot wipe the customisations of your old one. Changing a value back to its default on purpose counts as an edit like any other. The Mac that synced first keeps its data: after its second sync (about 30 seconds) its items count as made at its first sync, which beats every Mac that joins later whatever their device ids, while any edit made after that still wins by its time. A built-in snippet or quicklink you deleted before turning sync on stays deleted when you add a Mac. Only if two Macs turn sync on for the first time before either has synced twice, with different versions of the same item, the one with the greater device id wins on both, so they still end up the same. A deletion is remembered for good, so an old Mac or a leftover folder in iCloud Drive cannot bring a deleted item back. Clipboard deletions are remembered for 400 days (at most 3,000 of the newest are kept), longer than the longest history setting, and a pinned clipboard entry older than 400 days is not synced, so edits to it stay on the Mac where you made them. While you are typing in the Notes window, merged notes wait until you pause for two seconds. Merging happens every 30 seconds, when you open the launcher and when you press **Sync now**. Turning sync off stops all reading and writing but leaves the folder in iCloud Drive as it is.

:::note[Clipboard history]
The history now leaves the Mac when sync is on, and it is not encrypted in iCloud Drive beyond what iCloud itself provides. Leave sync off if you do not want that.
:::
