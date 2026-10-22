<!-- Produced by the parent session after reading the redesign-existing-projects skill and rendering all seven Settings tabs (FERROCAST_VIEW=settings-tab-N scripts/shot.sh). The skills are web-oriented (CSS/HTML); their principles are applied to the egui UI, not their code. -->

# Settings window redesign

User request (verbatim): "vamos a mejorar el diseño de toda la ventana de opciones de la app usa las skill de frontend para mejorarlas y que sean completamente amigables para el usuario y profesionales".

## Audit (evidence: screenshots of the seven tabs, dark theme, 750x475 pt window)

| # | Finding | Where seen | Skill principle |
|---|---|---|---|
| A1 | Only ~3 rows are visible per screen: rows are 65-70 pt tall because the description column is narrow (about 46% of the card) so a description wraps to 2-4 lines, and some are clipped by the scroll edge. | General, Translate, Advanced | whitespace is not density; text width ~65 chars; align shared rhythm |
| A2 | Text fields are raw near-black boxes (Languages, Server) that match nothing else: other controls use the card-grey fill. No border, no focus ring, no placeholder styling, content truncated without hint. | Translate, AI | consistent surfaces; missing focus ring; no error/validation states |
| A3 | The last row of every card draws a trailing separator line, so each card ends with a doubled edge. | all cards | polish; uniform radius/lighting |
| A4 | Sidebar icons are text glyphs ("AI", "Tr", "···", "{}") on coloured chips: inconsistent weight and meaning; no grouping, no tooltips, no keyboard way to switch tabs. | sidebar | iconography consistency; keyboard operability; active state |
| A5 | "Settings" appears twice (header and footer); the footer only offers Back/esc. | header, footer | redundancy; contextual hints |
| A6 | Wide controls (4-5 option segmented bars, long text fields) sit beside long descriptions and squeeze them; no stacked layout for them. | After translating, Provider | layout: buttons/controls aligned, stacked when wide |
| A7 | No way to find a setting without knowing its tab; no per-tab reset; dependent rows stay fully active when their switch is off; no readiness status chips (signed in, key present). | all | professional ergonomics, states |
| A8 | Capitalisation mixes sentence case and Title Case ("Launcher hotkey" vs "Translate wheel key" ok, but "Check for updates automatically" etc.): keep sentence case everywhere. | all | content rules |

Not changed on purpose: the dark palette and the coral accent (one accent, no gradients), the 750x475 launcher window size (the whole app is tested against it; a bigger Settings window is a separate decision), every setting's meaning and storage.

## Units (sequential, one commit each)

### SD1. Row and control system (draw.rs helpers)
- Adaptive row layout: description column up to ~60% of the row (cap ~62 characters per line); rows whose control is wide (segmented with 4+ options, text fields, combos) stack the control under the text instead of beside it; trimmed paddings and type scale (title 13.5, description 12) so at least 5 rows fit a screen.
- One field style for every TextEdit/ComboBox in Settings: card-field fill, 1 px border that turns accent on focus, 8 pt radius, 28 pt min height, placeholder in the sub colour, same hover state as buttons.
- Remove the trailing separator of the last row of a card; uniform 12 pt card radius; sentence-case audit of Settings strings.
- Tests: render all tabs in both themes; a geometry test that the last row of a card has no separator; a test that every Settings text field/combo goes through the shared field helper (marker like the existing `segmented-count` pattern).

### SD2. Sidebar, header and footer
- Real icons: SF Symbols through the existing `sf:` symbol loader (gearshape, doc.on.clipboard, curlybraces, sparkles, character.bubble, terminal, slider.horizontal.3); keep the accent chip only for the active tab; add thin group dividers (Everyday: General, Clipboard, Snippets / Intelligence: AI, Translate / Power: Scripts, Advanced).
- Keyboard: Cmd+1..7 jump to a tab, Up/Down move between tabs when the sidebar has focus; visible focus ring on sidebar items; tooltip with the tab name and shortcut.
- Header shows the current tab as breadcrumb ("Settings / Translate") and the footer drops the duplicate title, showing contextual hints (Cmd+1..7 tabs, esc back).
- Tests: tab shortcuts switch tabs, the shortcuts do nothing outside Settings, rendering in both themes, sidebar item count/order.

### SD3. Find a setting
- A search field at the top of the sidebar (shortcut Cmd+F inside Settings): filters a static index of every setting row (tab, title, keywords such as "hotkey", "proxy", "model"); choosing a result switches tab, scrolls to the row and flashes it for one second. A test walks every tab, records every row title drawn (cfg(test) marker in the row helper) and asserts each is in the index, so a new setting cannot be forgotten.
- Empty state: "No setting matches 'xyz'" with a hint.

### SD4. Per-tab polish
- Dependent rows dim and disable when their parent switch is off (Drop wheel off dims its three keys and the translate key; Check for updates off dims Install mode; snippet expansion).
- Status chips (dot + text) for readiness: AI provider key/sign-in/installed, Accessibility granted, translation engine usable; chips replace prose sentences like "Claude Code is not installed.".
- Inline validation: languages count and duplicates (chip "8 languages"), hotkey conflict message, server URL shape; "Reset this tab" text button at the bottom of each tab with a one-step undo toast.
- Empty states for Snippets and Scripts (what it is, one action).

### SD5. Visual QA
- Screenshots of every tab in both themes (target/ui-proof/), contrast check of the sub text on card and field fills (WCAG AA 4.5:1 for 12 pt text, computed in a test from the palette), minimum hit target 28 pt for every control (geometry test over the recorded rects), update docs screenshots only if the user asks.

## Risks and order
SD1 changes shared helpers used by every tab (draw.rs `row_impl`, `group_rect`, `segmented_bar`): existing segmented/geometry tests are the safety net; SD2-SD4 build on it. Item 3 of the Risk list (consumers of shared helpers) applies: keep every existing Settings test green or change its expectation deliberately and list it. No data, security or delivery item.
