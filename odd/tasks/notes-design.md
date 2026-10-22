# Notes design: the Notes window in the style of the web and the app
Branch: feat/notes-design (worktree target/wt-notes-design), from origin/master 555f4d3. Visual only: no behavior change.

## Specs
- S1 User request (verbatim): "mejoremos el diseño de las notas para que sigan el estilo de la web y de la app"
- S2 Palette of the app (crates/app/src/ui.rs `palette(dark)`), the notes must use the same values, not system colors. Dark: bg rgba(24,24,27,248), border white a28, text gray 238, sub gray 140, sub_strong gray 166, accent_text rgb(255,112,122), sel white a24, accent rgb(240,83,95), field white a14, sep white a20, danger rgb(240,83,95). Light: bg rgba(250,250,251,250), border black a36, text gray 30, sub gray 120, sub_strong gray 88, accent_text rgb(190,28,52), sel black a18, accent rgb(232,54,75), field black a10, sep black a22, danger rgb(200,40,40). Radius 14 (`ui::RADIUS`).
- S3 Palette of the web (web/src/styles/site.css): --bg #0f0f11, --surface #19191c, --stage #232327, --ink #f4f4f6, --muted #a8a6ad, --accent #f0535f, --accent-ink #1a0709, --radius 10px, display font weight 300 with letter-spacing -.022em. The coral accent is the common thread of web and app.
- S4 The coral accent replaces the system accent color everywhere in Notes: text caret and selection, task checkboxes, links (accent_text), active Format Bar buttons, the selected row of the pop-ups and of the palette, focus rings.
- S5 Notes follow the app's theme setting (Settings.theme: System, Dark, Light), not only the macOS appearance.
- S6 Typography: the system font like the launcher (the app loads SF Pro and SF Mono); headings echo the web's display style (light weight, tight tracking -0.022em) while keeping the hierarchy readable at every zoom level (50% to 300%).
- S7 Contrast: text and controls keep at least 4.5:1 in both themes (the app already checks this for Settings), the label chips and the selected rows included.
- S8 Surfaces: window background, toolbar hairline, code block and inline code fills, quote bar, palette card and pop-ups all come from the S2 palette; inner shapes use a radius between 10 and 14.
- S9 No behavior change: every existing test and demo assertion keeps passing; only colors, fonts, spacing and radii change.

## Tasks
- T1 S2,S4,S5,S8 `crates/macos/src/notes_theme.rs`: the palette as dynamic NSColors (light and dark), an appearance setter (System, Dark, Light), the window chrome (background, toolbar, title, hairline, toolbar buttons tint) in notes.rs; a test that pins the palette values — commit: pending
- T2 S4,S6,S7 editor styling in notes_editor.rs: text, headings, quote bar, code block, inline code, links, checkboxes, caret and selection, list markers, rule — commit: pending
- T3 S4,S7,S8 overlays: notes_popups.rs (pop-up rows, Format Bar) and notes_palette.rs (card, rows, search field) — commit: pending
- T4 S5 app wiring in crates/app: apply the Settings.theme to the Notes window at startup and when it changes; a test that the app palette and the notes palette have the same values — commit: pending
- T5 S9 side-by-side screenshots of launcher and Notes in both themes, demos re-run, changelog line — commit: pending

## Log
- L1 (verbatim): "mejoremos el diseño de las notas para que sigan el estilo de la web y de la app"
- D1 Notes follow the app theme setting (S5). Reversible.
- D2 The app palette is the source for colors (the notes sit next to the launcher); the web contributes the accent, the radius and the heading style (S3, S6).
- D3 Font: the system font, as the launcher. Stack Sans is a web font (woff2) and is not bundled in the app.
- E1 Current state seen in the demo PNGs: system window background (pure white or near-black), system accent (orange on this Mac) for caret, selection, checkboxes and selected rows, blue links, plain toolbar.
