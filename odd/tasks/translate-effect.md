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
- T1 S3,S4,S5 macOS overlay (glow.rs: show/move/hide, NSPanel + Core Animation) + ax_text selection bounds — route: gentle-ai-worker — commit: pending
- T2 S2,S6 core JS builders: mark selection with the effect, replace the marked text, restore it — route: inline — commit: pending
- T3 S2-S6 app wiring: start at pick, end on every outcome, timeout, setting, docs — route: gentle-ai-worker — commit: pending

## Log
- L1 "funciona, pero debemos poner algo mientras la ia responde con la traduccion, podriamos crear un efecto alrededor del texto que vamos a traducir para indicar al usuario que se esta traducciendo? el efecto debe de ser moderno chulo y que quede bien, algo como un blur o algo con efecto apple"
- L2 Evidence: shelf.rs already builds an NSPanel with NSVisualEffectView; system::reduce_motion exists; no Core Animation binding and no AX bounds helper yet. T1 may add the objc2-quartz-core crate (same objc2 0.3 family) for CAGradientLayer/CABasicAnimation: dependency change (Risk item 5).
