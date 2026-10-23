---
title: "Notion: search and add"
description: "Search the Notion pages you share with Ferrocast, open them, and add typed text or the clipboard to a page or database. Uses an internal integration token."
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
| **Add Text Here…** | | Opens the **Add to Notion** form aimed at this result for one send. The saved capture target does not change. |

## Add to Notion

Two more commands send text to your **capture target**, the page or database chosen in Settings → Integrations → **Capture target** (or with **Set as Capture Target** above). Connect a token and choose a target first. Without a token the toast says "Connect Notion first: Settings → Integrations." and opens that tab; with a token but no target it says "Choose a capture target in Settings → Integrations." and opens it too.

### Add to Notion (form)

Opens a form with a **Title** (optional) and a multi-line **Text** box. The cursor starts in the text.

| Key | What it does |
|---|---|
| <kbd>↵</kbd> in **Title** | Moves to the text box. |
| <kbd>⌘</kbd> <kbd>↵</kbd> | Sends. The footer shows **Send ⌘↵**. |
| <kbd>esc</kbd> | Goes back. Nothing is kept. |

When Notion accepts the text, the toast says "Added to" followed by the target name and the form is cleared. If the send fails, the toast shows what went wrong and the form keeps your text, so you can fix the problem and press <kbd>⌘</kbd> <kbd>↵</kbd> again.

### Add Clipboard to Notion

Sends the text on the clipboard right away, with no form, to the same target. If the clipboard is empty or holds something that is not text (an image or files), the toast says "The clipboard has no text." and nothing is sent.

### Page or database target

- **Page**: the text is appended at the end of the page as Markdown. A title, if you typed one, goes first as a `## Title` heading. Existing content is never changed.
- **Database**: Ferrocast creates a new page (row). Its name is the **Title** you typed; with no title it is the first non-empty line of the text, cut to 100 characters, and the body is the rest of the text. When the first line is longer than 100 characters the whole text stays in the body, so the cut loses nothing. The title column of each database is read once from Notion and remembered until you quit Ferrocast.

Markdown in the text (headings, lists, links, code blocks) becomes Notion blocks.

### Very long text

Text over 100,000 characters is split at blank lines into consecutive parts of at most 100,000 characters and sent one after another. A fenced code block is not split unless it alone exceeds the limit; in that case it is cut by lines and the fence is closed and reopened so each part stays valid. If a later part fails, the toast says how many parts were added ("Added 1 of 3 parts to …; the rest were not sent.") and your text stays in the form. Sending it again would repeat the parts already added, so remove them first or send only the rest.

If Notion answers that it is rate limiting requests, Ferrocast waits one second and tries that request once more.

## Privacy

- The token is stored in the macOS Keychain. It is never written to `settings.json`, logs, or the command line of any process.
- Ferrocast only talks to `api.notion.com`, and only when you open Search Notion, type in it, send with Add to Notion or Add Clipboard to Notion, or use the Settings → Integrations buttons.
- What you type in the search box, the titles of pages Notion returns, and the text you choose to add travel to and from Notion. Nothing else is sent.
- Search Notion reads and opens pages. Add to Notion only adds: it appends to a page or creates a new page in a database, and never edits existing text or deletes anything in your workspace.

## Troubleshooting

| Message | What to do |
|---|---|
| **Notion rejected the token. Paste a new one in Settings → Integrations.** (401) | The token was revoked or mistyped. Copy it again from notion.so/my-integrations and paste it in Settings → Integrations. |
| **Notion cannot see that page. Share it with your integration (••• → Connections).** | The page is not shared with the integration. In Notion choose **•••** → **Connections** and add it. |
| **The Notion integration lacks permission for this.** | The integration is missing a capability or access to that page. Check its capabilities at notion.so/my-integrations and add it to the page under **Connections**. |
| **Notion is rate limiting requests. Try again in a few seconds.** | Notion allows only a few requests per second. Wait a moment and search again. |
| **Could not reach Notion. Check your connection.** | You are offline or something blocks `api.notion.com`. The list stays as it was, so you can try again. |
| Nothing is listed | Check that you shared at least one page, then press **Test connection** in Settings → Integrations. |
| **Connect Notion first: Settings → Integrations.** | No token is saved yet. Paste one in Settings → Integrations. |
| **Choose a capture target in Settings → Integrations.** | Pick the page or database under **Capture target**, or use **Add Text Here…** on a search result. |
| **The clipboard has no text.** | Copy some text first. Images and files cannot be sent. |
| "Added 2 of 3 parts … the rest were not sent." | A long text stopped midway. Your text is still in the form; remove the part already in Notion, or paste only the remainder, and send again. |
| The page is added but the title is odd | In a database, the first line is the title unless you fill **Title**. Type a title in the form to choose it. |
