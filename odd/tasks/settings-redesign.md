# Settings window redesign
Branch: feat/settings-redesign · Plan: odd/plans/settings-redesign-plan.md · Status: in progress

## Specs
- S1 "mejorar el diseño de toda la ventana de opciones de la app" (all seven tabs).
- S2 "usa las skill de frontend": principles from redesign-existing-projects applied to the egui UI (audit A1-A8 in the plan).
- S3 "completamente amigables para el usuario y profesionales": discoverable (search, grouping, keyboard), clear states (dependent rows, status chips, validation, empty states), accessible (contrast, hit targets, focus).
- S4 No setting changes meaning or storage; palette and window size stay.

## Tasks
- T1 S1,S4 SD1 row and control system — commit: see git log (SD1)
- T2 S1,S3 SD2 sidebar, header, footer — commit: see git log (SD2)
- T3 S3 SD3 settings search — commit: see git log (SD3)
- T4 S3 SD4 per-tab polish — commit: see git log (SD4)
- T5 S3 SD5 visual QA — commit: see git log (SD5)

## Log
- L1 "vamos a mejorar el diseño de toda la ventana de opciones de la app usa las skill de frontend para mejorarlas y que sean completamente amigables para el usuario y profesionales"

- L2 SD1 done (worker + parent tuning): adaptive rows (text column up to 372 pt, control stacks under the text when it leaves <170 pt or the description would wrap to >3 lines), one field style (card fill, accent border on focus, 28 pt), no trailing separator, 12 pt card radius, 'Root search' sentence case. RED 101 seen; workspace 337 app / 356 core / 54 macos green, fmt/clippy 0. Screenshots in /tmp/ds-sd1/ (before/after, v4). Open: hint contrast (SD5), small_btn 26 pt vs 28 pt fields, AI tab below the fold not seen.

- L3 SD2 done (worker, parent re-checked): SF Symbol icons, two group dividers, Cmd+1..7 and Up/Down tab navigation, focus ring and tooltips, 34 pt items, breadcrumb 'Settings / <Tab>', footer chips 'Tabs Cmd+1-7' and 'Back esc'. RED 101 (7 of 8 tests), workspace 345 app / 356 core / 54 macos green, fmt/clippy 0. Not verifiable headlessly: tooltip, focus ring look, real keyboard focus. Open: inactive icon contrast (SD5), divider narrower than items, bare footer left (only app icon).

- L4 SD3 done (worker, parent re-checked): core settings_index (39 entries, ranked search, cap 8) + search field above the sidebar (Cmd+F), results pane with Up/Down/Enter/click, Escape clears the query first, open-result = switch tab + scroll + 1 s flash, empty state, coverage test that every drawn row title is in the index and vice versa. RED 101 (core 7/8, app 9/9); workspace 354 app / 364 core / 54 macos green, fmt/clippy 0. Open: active tab stays highlighted while results show, whole-title match is all red, no Cmd+F hint, scroll/flash only asserted by records (not visible headlessly).

- L5 SD4 done (worker, parent re-checked and tuned): no active tab while searching + 'Settings / Search', dependent rows dimmed and disabled (drop wheel -> wheel keys + translate key, update check -> install mode, clipboard history -> retention), status chips (AI/Translate provider readiness, Accessibility), inline validation (languages note, server URL), Reset this tab with 8 s Undo for General/Clipboard/Translate/Advanced, empty states for Snippets/Scripts, After translating copy shortened, Find Cmd+F footer chip. Parent fix: a text field's wanted width is learnt by row_flow so a stacked Languages field is no longer cut off. RED 101 (app 20/23, core 1); workspace 377 app / 365 core / 54 macos green, fmt/clippy 0. Not verifiable headlessly: Reset link on long tabs, footer Undo link, Translate engine chip, real side effects of a reset.

- L6 SD5 done (worker, parent re-checked): WCAG contrast test over every Settings text role in both themes (>=4.5 for text, >=3 for icons/hints/disabled; new Palette::sub_strong and accent_text, disabled opacity 0.72), 28 pt hit targets (small_btn, pill, segmented cells, toggle hit rect, back/clear buttons), shared focus ring on every control, Reset link styled as an action, search magnifier 15 pt, 'Ferrocast' in the Settings footer. RED 101 (3 of 5 tests; the worker reverted key values to take RED, so not strictly test-first). Workspace 383 app / 365 core / 54 macos green, fmt/clippy 0. Known leftovers: white-on-coral buttons are 3.44:1 dark / 4.15:1 light (below 4.5, accent kept on purpose), combo dropdown arrow is a heavy triangle, Reset link below the fold on long tabs, focus ring look and real keyboard focus not verified.
