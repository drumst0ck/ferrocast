# Translate wheel for dragged text
Branch: feat/translate-wheel · Plan: odd/plans/text-translate-wheel-plan.md · Status: in progress

## Specs
- S1 Drag selected text, tap the Convert key, a ring of target languages opens; dropping on one translates the text.
- S2 Engine is AI only, through the chat's `ai_stream`: OpenAI-compatible, Anthropic, ChatGPT subscription, Claude Code subscription. Setting `translate_provider` ("Same as chat" default).
- S3 Settings: languages (max 12), after translating (Replace/Copy), translation engine in the AI tab.
- S4 File wheels keep working exactly as before. Dragged text is never logged.

## Tasks
- T1 S1,S4 core wheel.rs has_text + Translate — commit: e9805c1
- T2 S1,S3 core translate.rs + settings fields — commit: b90fd13
- T3 S1 macos drag_text — commit: bba41d7
- T4 S1,S2,S4 app wiring — commit: e1709f1
- T5 S3 settings UI — commit: 7394414
- T6 docs — commit: docs(translate) (git log)

## Log
- L1 "traza el pan que ejecutara sonnet, la idea es que al igual que tenemos la ruleta para transformar archivos al seleccionar texto podamos traducirlo al idioma que queramos"
- L2 "vamos a dejar el translate con ia como dijiste al principio, pero que pueda usar tu sub de claude o openai almigual que el chat con ia que ya tenemos inolementado y incluir en opciones que quieres usar como motor de traduccion"
- L3 "ejecutalo"

- L4 Slice boundary: TW1+TW2 (459 lines) form slice 1; TW3-TW4 are slice 2, TW5-TW6 slice 3. Push/PR stay the user's decision.

- L5 Deviations: dragged text lives in App.wheel_text (not a WheelSubject enum in View::Wheel); no system-prompt override (the instruction is in the user turn, SYSTEM_PROMPT unchanged). Proven: workspace fmt/clippy exit 0, tests 292 app + 331 core + 52 macos green, file-wheel tests unchanged. NOT proven (needs a real session): H1 which apps put plain text on the drag pasteboard, H2 selection replaced after drop and focus return, H3 Copy-only, H4/H4b real provider calls (Ollama down, ChatGPT/Claude Code), H5 file wheels in a live drag, H6 over-limit toast. Web docs not built. Risk: item 3 (shared Tracker/Sample and wheel_segments, covered by the existing file-wheel tests).
