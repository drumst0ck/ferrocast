# Onboarding tour
Branch: feat/onboarding-tour · Status: in progress
A first-run presentation inside the app (egui, fixed 750×475 window) that shows what to enable and everything the app can do, styled like ferrocast.app.

## Specs
- S1 User (verbatim, L1): "cuando alguien lo instala por primera vez, una presentacion"
- S2 User (verbatim, L1): "que te indique todo lo que debe activar"
- S3 User (verbatim, L1): "todo lo que puede hacer la app"
- S4 User (verbatim, L1): "debe tener un diseño chulo y atractivo como la web"
- S5 User (verbatim, L1): "usa la skill de frontend para hacerlo"

## Tasks
- T1 S1,S4,S5 deck framework: View for the tour (slide index, eased slide transition, staggered reveal), keys ←/→/Enter/Esc, clickable dots, first-launch trigger replaces the old auto-open of Welcome (`settings.onboarded`, `--hidden` still suppresses), re-openable from search — route: gentle-ai-worker — commit:
- T2 S3,S4,S5 slides: what the app does (search + calculator, drag wheel and file tools, clipboard/snippets/quicklinks/scripts, windows, Ask AI + translate) with real product imagery, keycaps, large type; one claim per slide — route: same worker — commit:
- T3 S2 permissions slide: Accessibility (live status, grant button), Input Monitoring and Automation for Finder (what each unlocks, what works without it, open the System Settings pane), and the ⌥ Space hotkey — route: same worker — commit:
- T4 S1-S4 tests (render each slide, navigation, first launch opens it once, `--hidden`), docs (first-run.md en/es, CHANGELOG), screenshots checked with scripts/shot.sh — route: inline verify — commit:

## Design notes (not specs)
- design-taste-frontend is a website skill (its scripts need a browser); its principles are applied by hand: one claim per slide, an idea taken from the subject (iron + cast, the red accent, keycaps), type set large, whole fields of colour, real screenshots of the app instead of icon-card rows.
- Palette and tokens: crates/app/src/ui.rs (accent rgb 240,83,95). Window cannot resize per view: everything fits 750×475.

## Log
- L1 "ahora, lo que deberiamos preparar es cuando alguien lo instala por primera vez, una presentacion, que te indique todo lo que debe activar y todo lo que puede hacer la app, debe tener un diseño chulo y atractivo como la web, usa la skill de frontend para hacerlo"
- L2 Evidence (explore): View::Welcome exists (draw.rs ~5052) with Accessibility only; trigger app.rs:1061; no Input Monitoring or Automation checks exist; no easing helper beyond egui animate_*; assets are PNG only (image crate png feature); web/public/img has real screenshots (home.jpg, files-wheel-convert.jpg…); scripts/shot.sh renders a view to PNG.
- T1-T3 implemented (writer): `crates/app/src/tour.rs` (deck, 7 slides, eased transition, staggered reveal, clickable dots), `View::Welcome` kept as the tour view; first launch via `tour::first_run` (once, `--hidden` neither opens nor uses it up); footer chips Next/Start · Navigate ←/→ · Skip esc; the tour has no search bar (full-bleed). Menu title is now "Welcome tour & Permissions" with keywords tour/presentation/intro added.
- Permissions slide is slide 5 and the cheat sheet slide 6 (order as specified, not swapped). Accessibility is live (`accessibility_trusted(false)` polled every 0.5 s while visible); Input Monitoring is live through the new `windows::input_monitoring_granted()` (CGPreflightListenEventAccess, silent); Automation for Finder shows "Asked on first use" with no status claim. Enter never prompts; the grant is a button.
- Images: resources/tour/{home,files-wheel-convert,ai}.png (~156 KB), converted from web/public/img with sips. Dev hook: `FERROCAST_VIEW=welcome FERROCAST_TOUR_SLIDE=<0-6>` (use `FERROCAST_REDUCE_MOTION=1` so the screenshot is the settled frame).
- Existing test untouched: `View::Welcome.footer_hints()` stays empty; the tour chips come from `Tour::hints()` in the footer.
- Verified numbers against README.md: 275 conversions, 26 file tools, ~40 window layouts. Open for T4: docs/CHANGELOG, first-run.md en/es.
