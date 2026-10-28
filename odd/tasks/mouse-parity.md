# Mouse parity: everything the keyboard does, a click can do
Branch: feat/mouse-parity · Status: in progress
Every action reachable with a keyboard shortcut in Ferrocast must also be reachable with the mouse or trackpad, starting with the clipboard history (delete one entry, clear history).

## Specs
- S1 Notion "Mejoras Ferrocast / limpiar historial" (verbatim): "poder limpiar el historial del portapapeles y poder eliminar solo lonque queramos del portapapeles"
- S2 User (verbatim, L2): "que tambien funcione con click, en general todo lo que hacemos en la app con comandos de teclado debe poder hacerse tambien con clicks, revisa todas las funciones para que asi sea"
- S3 Keyboard behaviour stays exactly as it is; mouse paths are added, never replace keys.

## Tasks
- T1 S2 launcher footer: the "Actions ⌘K" hint opens the action panel on click, and the other footer hints (Back esc, primary action ↵) are clickable, following the Notes HintBar pattern. — done (L4)
- T2 S2 right-click (secondary click) on any launcher row selects it and opens the action panel for that item (the missing context menu); double-click runs the primary action. — done (L4)
- T3 S2 action panel: clicking outside it closes it. — done (L4)
- T4 S1,S2 hover buttons on rows for the common per-item actions: clipboard (pin, delete), snippets and quicklinks (edit, delete), notes; a visible "Clear History…" entry for the clipboard view with confirmation, and a "+" for New in snippets/quicklinks. — done (L4; Notes rows not covered, left for T5)
- T5 S2 audit the unverified areas from L3 (Notes ⌘1–9 and ⌘[ ⌘], PDF organizer keys, video editor shortcuts without buttons, Tour arrows, wheel segment clicks) and add click paths where missing. — pending
- T6 docs (keyboard-shortcuts page mentions the mouse paths; clipboard page), CHANGELOG, tests. — done (L4)

## Log
- L1 Notion idea "limpiar historial" (Alta).
- L2 User (verbatim): "que tambien funcione con click, en general todo lo que hacemos en la app con comandos de teclado debe poder hacerse tambien con clicks, revisa todas las funciones para que asi sea"
- L3 Evidence (explore subagent): launcher rows run the primary action on click (draw.rs:1920-1926), double-click ignored (draw.rs:1917-1919); ⌘↵ secondary action has no click path; the action panel rows are clickable (draw.rs:2308-2330) but the panel cannot be opened by mouse — footer "Actions ⌘K" is painted only (draw.rs:2059, draw_footer ~1940); no right-click anywhere (launcher or Notes); panel has no outside-click close (draw_panel draw.rs:2293); clipboard pin/delete/clear only via panel (app.rs:4715-4718); snippets/quicklinks edit/duplicate/delete/new only via keys/panel (app.rs:4680-4709, draw.rs:965-969); Notes footer hints already clickable (notes_hints.rs:149-199); Settings sidebar clickable; editors partly covered.
- L4 Implementation (writer subagent, T1-T4, T6; not committed). Footer hints ⌘K/↵/esc are click buttons that stand in for their key (a pending key read by `read_keys`), so behaviour is the key path itself (S3). Right click on a row selects it and calls `open_panel` (shared with ⌘K). Outside click (primary or secondary) closes the panel and swallows the row click under it; footer hints are excluded so Actions toggles. Row hover buttons (selected row) come from `App::row_buttons`, which picks the `Act` out of `actions_for` (Clip: Pin/Unpin, Delete; Snippet/Quicklink: Edit, Delete); tooltips show name + shortcut. `App::header_buttons`: Clear History… (Act::ClipDeleteAll through `perform`, so same Confirm, pinned kept) and + New (Act::CreateSnippet / CreateQuicklink), drawn at the right end of the search bar. Double click: the first click already runs the primary action, so double maps onto the same path (no second run). Tests: `mouse_parity::*` (10) in tests.rs. RED: first run failed to compile (row_buttons/header_buttons missing); GREEN after implementation. `cargo test -p ferrocast`: 971 passed, 0 failed, 36 ignored; clippy clean; fmt --check OK. Not checked visually: pill/hover rendering, tooltip placement, header buttons overlapping a very long query. Notes window, editors, Settings untouched (T5 pending).
