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
- T3 S3 SD3 settings search — commit: pending
- T4 S3 SD4 per-tab polish — commit: pending
- T5 S3 SD5 visual QA — commit: pending

## Log
- L1 "vamos a mejorar el diseño de toda la ventana de opciones de la app usa las skill de frontend para mejorarlas y que sean completamente amigables para el usuario y profesionales"

- L2 SD1 done (worker + parent tuning): adaptive rows (text column up to 372 pt, control stacks under the text when it leaves <170 pt or the description would wrap to >3 lines), one field style (card fill, accent border on focus, 28 pt), no trailing separator, 12 pt card radius, 'Root search' sentence case. RED 101 seen; workspace 337 app / 356 core / 54 macos green, fmt/clippy 0. Screenshots in /tmp/ds-sd1/ (before/after, v4). Open: hint contrast (SD5), small_btn 26 pt vs 28 pt fields, AI tab below the fold not seen.

- L3 SD2 done (worker, parent re-checked): SF Symbol icons, two group dividers, Cmd+1..7 and Up/Down tab navigation, focus ring and tooltips, 34 pt items, breadcrumb 'Settings / <Tab>', footer chips 'Tabs Cmd+1-7' and 'Back esc'. RED 101 (7 of 8 tests), workspace 345 app / 356 core / 54 macos green, fmt/clippy 0. Not verifiable headlessly: tooltip, focus ring look, real keyboard focus. Open: inactive icon contrast (SD5), divider narrower than items, bare footer left (only app icon).
