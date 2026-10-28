# Custom themes
Branch: feat/custom-themes · Status: in progress
Let the user customise Ferrocast's colours with an editor in Settings: accent, background, text and surfaces, with a live preview; themes are saved and can be exported and imported as files.

## Specs
- S1 Notion "Mejoras Ferrocast / custom themes" (verbatim): "poder hacer custom themes para la app, poder customizar la app colores y demas"
- S2 User decision (L2): "Editor de colores en Ajustes" — "Acento, fondo, texto y superficies, con vista previa; temas guardados y exportables como archivo."
- S3 The existing System / Dark / Light choice keeps working; a custom theme is an additional choice built on a light or dark base.
- S4 Text must stay readable: warn when a chosen text/background pair has too little contrast (WCAG ratio below 4.5:1).

## Tasks
- T1 S2,S4 core: Theme model (name, base light/dark, accent, background, surface, text, secondary text, separator), validation, contrast ratio, JSON import/export with versioning. — done (L3)
- T2 S2,S3 app: the launcher palette reads the active custom theme (every place that uses the hard-coded palette); Settings → General (Appearance) gains a theme list, a colour editor with pickers and a live preview, duplicate/rename/delete, Export…/Import…; the Notes window follows the theme where it already follows light/dark. — pending
- T3 docs (en/es), CHANGELOG, tests, screenshots of the launcher with two themes. — done (L3)

## Log
- L1 Notion idea "custom themes" (Baja).
- L2 User chose "Editor de colores en Ajustes".
- L3 Implemented T1-T3 (uncommitted). Core: `crates/core/src/themes.rs` (+`themes_tests.rs`, 12 tests, RED observed as 11 failures against todo!() stubs, then GREEN) and `Settings.custom_theme: Option<String>`. App: `ui::palette_custom`, `Palette` gained `hover/on_accent/popover/disc/custom` + `lift(k)` (default values byte-identical, tested), `App.custom_themes` + `active_theme()`, `theme_ctl.rs` (select/duplicate/rename/delete/import/export), Settings → General rows "Custom themes" + editor + preview + contrast warnings; themes in `themes.json`. Notes follows the theme's base through `set_theme` (no change needed in notes_theme.rs). Docs en/es `launcher/themes.md`, settings mentions, CHANGELOG, index row "Custom themes". Not themed on purpose: status-chip colours, recording-editor preview surface, welcome tour slides, tour/intro art. Open: ThemeAct clearing of custom theme by the System/Dark/Light segmented control is covered by code path only (no test); native save/open dialogs (osascript) not exercised by tests. Screenshots in /tmp/ftheme/.
