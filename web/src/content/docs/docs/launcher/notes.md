---
title: "Notes"
description: "A floating rich-text notes window: Markdown, images, a slash menu, pinning, export, AI commands and dictation, opened with a hotkey."
sidebar:
  order: 14
---
Notes is a rich-text window that floats above your other apps. Open it with a hotkey, write, and close it again. Notes are stored as Markdown in `notes.json` in the data folder.

## Commands

| Command | What it does |
|---|---|
| **Notes** | Shows or hides the Notes window. Searching “floating notes” finds it too. |
| **Create Note** | Creates a new note and opens the window. |
| **Search Notes** | A list of all your notes, searched by title and content. Opening one shows it in the floating window. |
| **Create Dictated Note** | Creates a new note and starts dictation. |

Each command takes its own hotkey: select it in the launcher and press <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>,</kbd>, or choose **Set Hotkey…** in the action panel (<kbd>⌘</kbd> <kbd>K</kbd>).

## Write a note

Press <kbd>⌘</kbd> <kbd>N</kbd> in the window, or use the plus button in the toolbar, to start a new note. The first line is the title, and it is shown in the toolbar. Only one note is visible at a time.

An empty note is discarded when you hide the window, unless it is your only note. Changes are saved a moment after you stop typing, and also when the window hides.

## Format text

There are five ways to format:

1. Keyboard shortcuts (tables below).
2. Markdown typing: type the Markdown and it turns into formatting.
3. The `/` menu: type `/` to pick a format from a list.
4. **Format** in the action panel (<kbd>⌘</kbd> <kbd>K</kbd>).
5. The Format Bar at the bottom of the window. It shows by default. **Hide Format Bar** in the action panel turns it off, and **Show Format Bar** brings it back.

A footer at the very bottom of the window lists the main shortcuts: New <kbd>⌘</kbd> <kbd>N</kbd>, Browse <kbd>⌘</kbd> <kbd>P</kbd>, Pin <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>P</kbd> and Actions <kbd>⌘</kbd> <kbd>K</kbd>. You can click the items in it, and in a narrow window the least important ones drop out.

The Notes window uses the same colors as the launcher and follows the Appearance setting in Settings (System, Dark or Light).

### Paragraph formats

| Format | Shortcut | Markdown |
|---|---|---|
| Heading 1 | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>1</kbd> | `#` and a space |
| Heading 2 | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>2</kbd> | `##` and a space |
| Heading 3 | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>3</kbd> | `###` and a space |
| Code block | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>C</kbd> | three backticks and a space, or `~~~` |
| Blockquote | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>B</kbd> | `>` and a space |
| Ordered list | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>7</kbd> | `1.` and a space |
| Bullet list | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>8</kbd> | `*` or `-` and a space |
| Task list | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>9</kbd> | `[]` or `[x]` and a space |
| Horizontal rule | none | `---` or `___` and a space |

To check or uncheck a task, put the cursor on its line and press <kbd>⌘</kbd> <kbd>↵</kbd>. Lists can be indented.

### Text formats

| Format | Shortcut | Markdown |
|---|---|---|
| Bold | <kbd>⌘</kbd> <kbd>B</kbd> | `**text**` or `__text__` |
| Italic | <kbd>⌘</kbd> <kbd>I</kbd> | `*text*` or `_text_` |
| Strikethrough | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>S</kbd> | `~~text~~` |
| Underline | <kbd>⌘</kbd> <kbd>U</kbd> | none (saved as `<u>text</u>`) |
| Code | <kbd>⌘</kbd> <kbd>E</kbd> | `` `text` `` |
| Link | <kbd>⌘</kbd> <kbd>L</kbd> | `[text](url)` |

Links are not in the `/` menu. Links open with <kbd>⌘</kbd>-click.

### The `/` menu and emoji

Type `/` to open a menu of formats and filter it by typing. Type `:` to open an emoji list and filter it the same way.

## Images

Put an image in a note in any of three ways:

- Paste it with <kbd>⌘</kbd> <kbd>V</kbd>: a screenshot, an image copied from a browser, or image files copied in Finder.
- Drag image files onto the note. They land after the line where you drop them.
- Choose **Insert Image…** (<kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>I</kbd>, or the right-click menu) and pick files.

PNG, JPEG, GIF, HEIC and WebP work, up to 20 MB each. A bigger file or one that is not an image is refused with a message that names it; the other images you added are still inserted. If you paste text that has a picture beside it (from a web page or a document), the text is pasted as before.

Ferrocast copies each image into the `attachments` folder of the data folder under a name made from its content, so the same image is stored once. The note gets a standard Markdown link on its own line, such as `![name](attachments/3f2a….png)`, and the image is drawn in the note at its own size, up to 480 points wide, with its proportions kept. Notes stay plain Markdown: the link is a relative path and only files in that folder are ever shown.

To remove an image, select it and press Delete. A file that no note links any more is deleted 7 days after its link was removed, however old the file is, so undo and Recently Deleted still find it; linking it again cancels that. If a file is missing (for example it has not reached this Mac yet) a placeholder is shown, and the picture appears once the file is here.

The Notes window edits only Ferrocast's own notes. Notes in your Obsidian vault are not edited here, so Ferrocast never copies an image into your vault, and it never moves images from there into its own folder.

With Sync with iCloud on, note images travel through iCloud Drive as separate files. Each Mac uploads only its own. A missing image is downloaded when a synced note links it, and an image is not removed while any Mac's notes still link it.

## Organize notes

- **Browse Notes**: <kbd>⌘</kbd> <kbd>P</kbd> or the toolbar button lists every note.
- **Pin**: <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>P</kbd> pins or unpins the open note (<kbd>⌘</kbd> <kbd>.</kbd> in the Search Notes list). Pinned notes sit at the top of Browse Notes and Search Notes.
- **Pinned notes**: while the window is open, <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>⌘</kbd> <kbd>9</kbd> open the first to the ninth pinned note.
- **History**: <kbd>⌘</kbd> <kbd>[</kbd> and <kbd>⌘</kbd> <kbd>]</kbd> go back and forward between the notes you opened, like a browser.

## Delete notes

In the action panel, **Delete Note** and **Delete All Notes** move notes to **Recently Deleted**. Open Recently Deleted from the same panel and choose a note to restore it. Deleted notes are kept for 30 days and then removed for good.

## Zoom

<kbd>⌘</kbd> <kbd>+</kbd> zooms in, <kbd>⌘</kbd> <kbd>-</kbd> zooms out and <kbd>⌘</kbd> <kbd>0</kbd> goes back to actual size. The zoom also lives in Settings → Notes.

## With the mouse

Every shortcut of the window has a click path. The toolbar has **‹** and **›** (back and forward through the notes you opened), **Browse**, **+** (new note) and **×** (close). The footer has **Actions**, **Browse**, **Pin** and **New**. Right-click the editor, the toolbar or an empty part of the window for a menu with New Note, Browse Notes, Pin or Unpin, Insert Image, Back, Forward, Zoom In, Zoom Out, Actual Size, Actions and Close; in the editor it comes after the usual Cut, Copy and Paste entries. Pinned notes are listed first in **Browse Notes**: click one to open it, as <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>⌘</kbd> <kbd>9</kbd> does. In **Search Notes**, hover a row for **Pin** or **Unpin** and **Delete**, and click **+ New** for a new note.

## The action panel

<kbd>⌘</kbd> <kbd>K</kbd> opens the action panel for the open note. Type to search it, including the items inside its submenus (for example Format › Bold or Export › Markdown). It holds:

- New Note, Browse Notes, Pin or Unpin Note, Navigate Back and Navigate Forward.
- Format, Show or Hide Format Bar and Start or Stop Dictation.
- Copy As, Export, Create Quicklink and AI Commands.
- Zoom In, Zoom Out and Actual Size.
- Delete Note, Delete All Notes and Recently Deleted.

Actions that do not apply are left out of the list.

## Export and copy

**Export** writes the note as **Plain Text**, **Markdown** or **HTML**. The file goes to your Downloads folder and is revealed in Finder; there is no save dialog. **Copy As** puts the note on the clipboard in the same three formats. None of them includes the image files of a note. Markdown and HTML keep the link (`attachments/…`, which only resolves inside Ferrocast's data folder), and Plain Text writes each image as `alt (attachments/…)`. Copying a selection with <kbd>⌘</kbd> <kbd>C</kbd> puts its Markdown on the clipboard when it contains an image, so cutting, copying, dragging and pasting a paragraph inside Notes keeps its picture; pasted into another app it is the Markdown text.

## Quicklink and URLs

**Create Quicklink** adds a [quicklink](/docs/launcher/quicklinks/) that opens that note from anywhere. It uses these URLs, which you can also use from other apps:

| URL | What it does |
|---|---|
| `ferrocast://notes/<id>` | Opens that note. An unknown or deleted note only shows a message. |
| `ferrocast://notes/new` | Creates a note and opens it. |
| `ferrocast://notes` | Shows or hides the Notes window. |

## AI commands

**AI Commands** in the action panel runs one of these on the selected text, or on the whole note when nothing is selected:

- Fix Spelling and Grammar, Improve Writing, Make Shorter, Make Longer.
- Change Tone: Professional, Casual, Friendly, Confident or Straightforward.
- Explain This and Translate to English.

**Summarize** and **Continue Writing** always work on the whole note.

They use the provider you set in Settings → AI (see [Ask AI](/docs/launcher/ask-ai/)) and send the text to it. Summarize and Continue Writing add their result at the end of the note, and Explain This adds it below the selection. They never replace your text. The other commands replace the text they worked on. Each result is applied as one undo step (<kbd>⌘</kbd> <kbd>Z</kbd>), and Markdown formatting is kept. One command runs at a time. If you edit the note while it runs, the result is discarded.

## Dictation

Choose **Start Dictation** in the action panel, or run **Create Dictated Note**, and speak. The first time, macOS asks for microphone and speech recognition access. Ferrocast uses on-device recognition when your system supports it. Dictation stops when you hide the window, switch notes, delete the note, edit the dictated text or choose **Stop Dictation**.

## Settings

Settings → Notes has three options:

| Option | What it does |
|---|---|
| **Show Notes Toggle Button in Menu Bar** | Adds a button to the menu bar that shows or hides the Notes window. Off by default. |
| **Escape Key Loses Focus** | Esc moves focus away from the Notes window instead of closing it. Off by default. |
| **Zoom** | Text size, from 50% to 300%. Default 100%. |

:::caution[Limits]
- There is no limit on the number of notes.
- <kbd>esc</kbd> closes the window unless you turn on **Escape Key Loses Focus**.
- <kbd>⌘</kbd> <kbd>0</kbd> means actual size, so pinned notes use <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>⌘</kbd> <kbd>9</kbd>.
- AI commands and dictation depend on your provider and on macOS permissions.
- Images are drawn on their own line or inline in a paragraph, never side by side with text wrapping around them, and cannot be resized or cropped in the note.
:::

## Related

- [Keyboard shortcuts](/docs/launcher/keyboard-shortcuts/)
- [Settings](/docs/launcher/settings/)
- [Quicklinks](/docs/launcher/quicklinks/)
- [Data and privacy](/docs/reference/data-and-privacy/)
