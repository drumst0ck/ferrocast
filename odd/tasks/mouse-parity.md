# Mouse parity: everything the keyboard does, a click can do
Branch: feat/mouse-parity · Status: in progress
Every action reachable with a keyboard shortcut in Ferrocast must also be reachable with the mouse or trackpad, starting with the clipboard history (delete one entry, clear history).

## Specs
- S1 Notion "Mejoras Ferrocast / limpiar historial" (verbatim): "poder limpiar el historial del portapapeles y poder eliminar solo lonque queramos del portapapeles"
- S2 User (verbatim, L2): "que tambien funcione con click, en general todo lo que hacemos en la app con comandos de teclado debe poder hacerse tambien con clicks, revisa todas las funciones para que asi sea"
- S3 Keyboard behaviour stays exactly as it is; mouse paths are added, never replace keys.

## Tasks
- T1 S2 launcher footer: the "Actions ⌘K" hint opens the action panel on click, and the other footer hints (Back esc, primary action ↵) are clickable, following the Notes HintBar pattern. — pending
- T2 S2 right-click (secondary click) on any launcher row selects it and opens the action panel for that item (the missing context menu); double-click runs the primary action. — pending
- T3 S2 action panel: clicking outside it closes it. — pending
- T4 S1,S2 hover buttons on rows for the common per-item actions: clipboard (pin, delete), snippets and quicklinks (edit, delete), notes; a visible "Clear History…" entry for the clipboard view with confirmation, and a "+" for New in snippets/quicklinks. — pending
- T5 S2 audit the unverified areas from L3 (Notes ⌘1–9 and ⌘[ ⌘], PDF organizer keys, video editor shortcuts without buttons, Tour arrows, wheel segment clicks) and add click paths where missing. — pending
- T6 docs (keyboard-shortcuts page mentions the mouse paths; clipboard page), CHANGELOG, tests. — pending

## Log
- L1 Notion idea "limpiar historial" (Alta).
- L2 User (verbatim): "que tambien funcione con click, en general todo lo que hacemos en la app con comandos de teclado debe poder hacerse tambien con clicks, revisa todas las funciones para que asi sea"
- L3 Evidence (explore subagent): launcher rows run the primary action on click (draw.rs:1920-1926), double-click ignored (draw.rs:1917-1919); ⌘↵ secondary action has no click path; the action panel rows are clickable (draw.rs:2308-2330) but the panel cannot be opened by mouse — footer "Actions ⌘K" is painted only (draw.rs:2059, draw_footer ~1940); no right-click anywhere (launcher or Notes); panel has no outside-click close (draw_panel draw.rs:2293); clipboard pin/delete/clear only via panel (app.rs:4715-4718); snippets/quicklinks edit/duplicate/delete/new only via keys/panel (app.rs:4680-4709, draw.rs:965-969); Notes footer hints already clickable (notes_hints.rs:149-199); Settings sidebar clickable; editors partly covered.
