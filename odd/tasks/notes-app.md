# Ferrocast Notes: a Raycast Notes equivalent inside the app
Branch: feat/notes (worktree target/wt-notes). Native macOS floating window with a real rich-text editor.
Reference for "same functions": https://manual.raycast.com/notes, https://www.raycast.com/blog/raycast-notes (read 2026-10-22).

## Specs
- S1 User request (verbatim): "bien, ahora vamos a desarrollar una app de notas dentro de nuestra app, exactamente igual que la que tiene Raycast notes, con exactamente las mismas funciones"
- S2 User decision, editor: "Nativo macOS (NSTextView) en un panel flotante" (rich text, notes stored as Markdown).
- S3 User decision, scope: "Quicklink a una nota", "Comandos de IA sobre la nota", "Dictado por voz", on top of the core.
- S4 Commands: "Raycast Notes: This command toggles the Notes window." / "Create Note: This command creates a new note and opens the window." / "Search Notes: This command allows you to search all your notes by title and content". Alias: searching "floating notes" finds the toggle command. Each command accepts a global hotkey.
- S5 Creating: "click the plus button in the top-right corner of the toolbar (or press ⌘N)". "The first line of the note is used as the title of the note, which is shown in the toolbar."
- S6 Formatting, five ways: Markdown typing, the `/` menu, keyboard shortcuts, the Format action in ⌘K, and the Format Bar at the bottom of the window.
- S7 Paragraph formats (shortcut / Markdown): Heading 1 ⌥⌘1 `#`; Heading 2 ⌥⌘2 `##`; Heading 3 ⌥⌘3 `###`; Code block ⌥⌘C "three backticks and a space" or "~~~"; Blockquote ⇧⌘B `>`; Ordered list ⇧⌘7 `1.`; Bullet list ⇧⌘8 `*` or `-`; Task list ⇧⌘9 `[]` / `[x]`. "To toggle a task's checked state, place the cursor on the task line and press ⌘Enter."
- S8 Text formats: Bold ⌘B `**x**` `__x__`; Italic ⌘I `*x*` `_x_`; Strikethrough ⇧⌘S `~~x~~`; Underline ⌘U (no Markdown); Code ⌘E `` `x` ``; Link ⌘L `[text](url)`. "Links aren't part of the `/` menu."
- S9 Other: inline emoji picker on typing `:`; horizontal rule on `---` or `___` and a space at the start of a line. Lists can be indented and reordered.
- S10 Organizing: "Only one note is visible at the time. Notes are organized as a stack." Browse Notes toolbar item or ⌘P.
- S11 Pinning: ⇧⌘P pins ("from the Search Notes window, it's ⌘."); pinned notes sit at the top of Search Notes and Browse Notes and open with ⌘0 through ⌘9 while the window is open.
- S12 Navigating: ⌘[ and ⌘] move between previously opened notes, like browser history.
- S13 Deleting: ⌘K → Delete Note, Delete All Notes. "notes move to Recently Deleted first rather than disappearing immediately."
- S14 Zoom: ⌘- / ⌘+ zoom out/in, ⌘0 actual size, and Settings → Notes → Zoom.
- S15 Settings: "Show Notes Toggle Button in Menu Bar"; "Escape Key Loses Focus" (Esc moves focus away instead of closing, for Notes only); "Zoom".
- S16 Export any note as plain text, Markdown or HTML. Create a Quicklink to a specific note that opens it from anywhere. All of these live in the ⌘K action panel with their shortcuts.
- S17 AI: "Use AI Commands inside Raycast Notes" such as "Fix Spelling and Grammar" and "Change Tone", using the AI providers Ferrocast already has.
- S18 Dictation: "Take notes hands-free using dictation"; a "Create Dictated Note" command opens a new note with recording on.
- S19 The window floats above other windows ("float above all windows and be fast to toggle with a hotkey").
- S20 Assumptions (see Log D1-D3): no note-count limit (Raycast's 5-note free cap is a Pro tier, Ferrocast has no tiers); existing `notes.json` notes keep working and appear in the new window; the old plain-text launcher views are replaced by the new commands.

## Tasks
- T1 S5,S10,S11,S12,S13,S16,S20 core notes model and store logic in `crates/core/src/notes.rs`: extra optional Note fields (pinned, deleted_at), title, search, pinned order, history stack, Recently Deleted + purge, export txt/md/html — worker (independent, pure) — commit: 3d7014f
- T2 S6-S9 core rich-text document in `crates/core/src/richtext.rs`: Markdown ⇄ block/inline model (headings, quote, bullet/ordered/task lists with levels, code block, rule, bold/italic/strike/underline `<u>`/code/link), lossless round trip — worker (independent, pure) — commit: pending
- T3 S2,S19,S5 macos window shell `crates/macos/src/notes.rs`: floating NSPanel + NSTextView in a scroll view, toolbar (title, browse, plus), toggle/focus/open, frame memory, plain text sync, PNG dump of the window for checks — inline — commit: fa19f00
- T4 S6-S9,S14 editor behavior: Doc ⇄ NSTextStorage, Markdown typing shortcuts, key shortcuts, list continue/indent, task toggle, links, `/` menu, `:` emoji, Format Bar, zoom — inline (after T2,T3) — commit: pending
- T5 S10-S13,S16 navigation and actions: Browse Notes ⌘P, new ⌘N, pin ⇧⌘P and ⌘0-9, history ⌘[ ⌘], ⌘K action panel, delete / delete all / Recently Deleted, export, copy — inline (after T1,T3) — commit: pending
- T6 S4,S15,S20 integration: Notes / Create Note / Search Notes commands + alias + hotkeys, menu bar toggle, settings (Escape loses focus, zoom, menu bar), replace the old launcher note views, settings index, shortcuts list — inline — commit: pending
- T7 S16 Quicklink to a note: `ferrocast://` URL scheme (Info.plist + Apple Event handler) and the create-quicklink action — inline (high risk: bundle/installer change) — commit: pending
- T8 S17 AI commands on the selection or the whole note via the existing providers, undoable — inline — commit: pending
- T9 S18 dictation: Speech + AVAudioEngine, microphone and speech usage strings in Info.plist, Create Dictated Note command — inline (high risk: permissions/bundle) — commit: pending
- T10 S4-S18 docs (EN/ES web docs, README, CHANGELOG, in-app help) and a final checks pass — inline — commit: pending

## Log
- L1 (verbatim): "bien, ahora vamos a desarrollar una app de notas dentro de nuestra app, exactamente igual que la que tiene Raycast notes, con exactamente las mismas funciones"
- L2 Answers to the two scope questions (verbatim): editor = "Nativo macOS (NSTextView) en un panel flotante"; extras selected = "Quicklink a una nota, Comandos de IA sobre la nota, Dictado por voz".
- L3 Exploration: the repo already has a plain-text notes feature (`Note { id, text, created, modified }` in `crates/core/src/models.rs`, `notes.json`, `View::Notes` / `View::NoteEdit` and `Builtin::SearchNotes` / `CreateNote` in `crates/app`); the new feature replaces its UI and keeps its data. No `ferrocast://` URL scheme exists yet. The shelf (`crates/macos/src/shelf.rs`) is the pattern for a native NSPanel with objc2.
- D1 The manual lists ⌘0 both as "actual size" (zoom) and as the first pinned slot. Chosen: ⌘0 resets zoom, ⌘1-⌘9 open pinned notes 1-9. Reversible; confirm with the user when the window can be shown.
- D2 Free tier cap of 5 notes not copied (S20).
- D3 Underline has no Markdown syntax in Raycast ("N/A"); stored as `<u>text</u>` so it survives a round trip.
- L4 T1 (3d7014f) and T3 (fa19f00) reviewed and re-run inline: macos tests, clippy and the notes_demo --check pass; light and dark PNGs inspected. The demo re-runs inside a temporary .app via open -W when the bare process cannot activate. Unverified: dragging the toolbar, frame restore after relaunch, real blur, spaces and full-screen.
