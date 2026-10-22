---
title: "Search Notion"
description: "Search the Notion pages and databases you share with Ferrocast, open them in Notion and copy their links. Connect with an internal integration token."
sidebar:
  order: 14
---
Search Notion finds your pages and databases from the launcher and opens them in Notion. It uses an internal integration token that you create yourself, so there is no sign-in window and no Ferrocast server in between.

## Set it up

1. Open [notion.so/my-integrations](https://www.notion.so/my-integrations) and create an **internal integration** for your workspace.
2. Copy its token (it starts with `ntn_`).
3. In Ferrocast, open Settings → Integrations, paste the token under **Notion token** and press <kbd>↵</kbd>. **Test connection** shows the integration and workspace name.
4. Share the pages you want to find. In Notion, open a page, choose **•••** → **Connections** and add your integration. Sharing a page also shares the pages inside it.

Notion only returns what has been shared with the integration. If a page is missing from the results, it has not been shared yet.

## Search

Open **Search Notion** from the launcher. With an empty search box it lists the 20 pages and databases edited most recently. Type to search; Ferrocast waits a quarter of a second after your last key and then asks Notion, so fast typing sends one request, not one per letter.

Each row shows the page icon (or a generic one), the title, whether it is a **Page** or a **Database**, and when it was last edited: "2h ago", "yesterday", or the date. When Notion has more results, the last row, **Load more**, adds the next 20. While a request runs the footer says "Searching Notion", and you can keep typing.

Without a token the view shows **Connect Notion to search your pages** and a single row, **Open Settings → Integrations**, which takes you to the token field.

You can give the command an alias or a hotkey like any other command, from <kbd>⌘</kbd> <kbd>K</kbd> in the launcher root.

## Actions

Press <kbd>⌘</kbd> <kbd>K</kbd> on a result.

| Action | Shortcut | What it does |
|---|---|---|
| **Open** | <kbd>↵</kbd> | Opens the page in Notion. The Notion desktop app handles the link when it is installed; otherwise your browser does. |
| **Copy Link** | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> | Copies the page URL. |
| **Set as Capture Target** | | Saves the page or database as the capture target, the same choice as Settings → Integrations → **Capture target**. |

## Privacy

- The token is stored in the macOS Keychain. It is never written to `settings.json`, logs, or the command line of any process.
- Ferrocast only talks to `api.notion.com`, and only when you open Search Notion, type in it, or use the Settings → Integrations buttons.
- What you type in the search box and the titles of pages Notion returns travel to and from Notion. Nothing else is sent.
- Search Notion reads and opens pages. It never changes or deletes anything in your workspace.

## Troubleshooting

| Message | What to do |
|---|---|
| **Notion rejected the token. Paste a new one in Settings → Integrations.** (401) | The token was revoked or mistyped. Copy it again from notion.so/my-integrations and paste it in Settings → Integrations. |
| **Notion cannot see that page. Share it with your integration (••• → Connections).** | The page is not shared with the integration. In Notion choose **•••** → **Connections** and add it. |
| **The Notion integration lacks permission for this.** | The integration is missing a capability or access to that page. Check its capabilities at notion.so/my-integrations and add it to the page under **Connections**. |
| **Notion is rate limiting requests. Try again in a few seconds.** | Notion allows only a few requests per second. Wait a moment and search again. |
| **Could not reach Notion. Check your connection.** | You are offline or something blocks `api.notion.com`. The list stays as it was, so you can try again. |
| Nothing is listed | Check that you shared at least one page, then press **Test connection** in Settings → Integrations. |
