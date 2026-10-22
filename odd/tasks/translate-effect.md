# Translating effect
Branch: feat/translate-effect · Status: in progress
Show the user that a translation is running, on the text being translated, with an Apple Intelligence / Writing Tools look.

## Specs
- S1 User (verbatim, L1): "funciona, pero debemos poner algo mientras la ia responde con la traduccion, podriamos crear un efecto alrededor del texto que vamos a traducir para indicar al usuario que se esta traducciendo? el efecto debe de ser moderno chulo y que quede bien, algo como un blur o algo con efecto apple"
- S2 Web pages (Safari, Chromium list in translate::browser_for, JavaScript from Apple Events on): the selected text itself shimmers in place (animated multicolour gradient across the text + soft glow), is replaced in place by the translation, and is restored untouched if the translation fails or is shown in the window.
- S3 Other apps where Accessibility reports the selection bounds: a click-through transparent overlay draws an animated glowing outline (rotating soft multicolour gradient, blurred halo) around the selection rectangle(s).
- S4 Otherwise: a small frosted-glass pill "Translating…" with the same glow at the drop point.
- S5 The effect starts when a language is picked and always ends: on delivery, on fallback, on error, and after a 60 s safety timeout. With macOS Reduce Motion on, the glow is static. A Settings → Translate toggle "Translating effect" (default on) turns it off.
- S6 Nothing is ever left modified in a page or on screen after the translation ends; the dragged text is never logged.

## Tasks
- T1 S3,S4,S5 macOS overlay (glow.rs: show/move/hide, NSPanel + Core Animation) + ax_text selection bounds — route: gentle-ai-worker — commit: done (see git log)
- T2 S2,S6 core JS builders: mark selection with the effect, replace the marked text, restore it — route: inline — commit: f6cf336
- T3 S2-S6 app wiring: start at pick, end on every outcome, timeout, setting, docs — route: gentle-ai-worker — commit: done (see git log)

## Log
- L1 "funciona, pero debemos poner algo mientras la ia responde con la traduccion, podriamos crear un efecto alrededor del texto que vamos a traducir para indicar al usuario que se esta traducciendo? el efecto debe de ser moderno chulo y que quede bien, algo como un blur o algo con efecto apple"
- L2 Evidence: shelf.rs already builds an NSPanel with NSVisualEffectView; system::reduce_motion exists; no Core Animation binding and no AX bounds helper yet. T1 may add the objc2-quartz-core crate (same objc2 0.3 family) for CAGradientLayer/CABasicAnimation: dependency change (Risk item 5).

- L3 T2 done inline: translate::web_mark_js / web_replace_marked_js / web_unmark_js. Design decision: the effect never rewrites page text while translating; it draws click-through overlay boxes (one per line, merged from getClientRects) with a hue-cycling glow (blue->purple->pink->orange, no green) and a tinted sweep, remembers the range, replaces it even if the page selection was lost, fades the new text in, and unmark removes only what it added. Verified in a real Chromium DOM through Playwright: mark 'ok' with page HTML unchanged, 6 client rects -> 3 line boxes, replace 'ok' after the selection was cleared, mark+unmark leaves document.body byte-identical, no selection -> 'nosel'; node --check on all three scripts; colour frames checked at 0/1300 ms. 32 translate tests green (RED 101 first).

- L4 T1 (bf912aa): glow.rs overlay NSPanel + Core Animation (conic ring + blurred halo, breathing, rotation; Reduce Motion static) and ax_text::selection_bounds; objc2-quartz-core/objc2-core-foundation were already transitive, no new crate in the lock. Look not captured (no Screen Recording for the terminal); the user ran glow_demo and liked it, asking for the website's neon red (#f0535f) instead of the multicolour palette (L5).
- L5 User (verbatim): "me gusta, pero podemos hacer que el glow sea del rojo neon que usamos en nuestra web para que tenga que ver con nuestra app y siga el mismo estilo".
- L6 T3 (worker, parent re-checked): translate_effect setting + Settings row + search entry, effect_start probe thread (web mark -> AX rects -> drop-point pill), run ids against races, end on every outcome incl. Escape/pop and a 60 s timeout, one log line per effect without text, Effects seam extended. RED 101; fmt/clippy 0; tests 393 app / 375 core / 64 macos green with a HOME without Claude Code. Not verifiable here: real look inside the app flow, real AX bounds, timeout while hidden.
