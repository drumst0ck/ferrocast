# Translate wheel for dragged text
Branch: feat/translate-wheel · Plan: odd/plans/text-translate-wheel-plan.md · Status: in progress

## Specs
- S1 Drag selected text, tap the Convert key, a ring of target languages opens; dropping on one translates the text.
- S2 Engine is AI only, through the chat's `ai_stream`: OpenAI-compatible, Anthropic, ChatGPT subscription, Claude Code subscription. Setting `translate_provider` ("Same as chat" default).
- S3 Settings: languages (max 12), after translating (Replace/Copy), translation engine in the AI tab.
- S4 File wheels keep working exactly as before. Dragged text is never logged.

## Tasks
- T1 S1,S4 core wheel.rs has_text + Translate — commit: see git log (TW1)
- T2 S1,S3 core translate.rs — commit: pending
- T3 S1 macos drag_text — commit: pending
- T4 S1,S2,S4 app wiring — commit: pending
- T5 S3 settings UI — commit: pending
- T6 docs — commit: pending

## Log
- L1 "traza el pan que ejecutara sonnet, la idea es que al igual que tenemos la ruleta para transformar archivos al seleccionar texto podamos traducirlo al idioma que queramos"
- L2 "vamos a dejar el translate con ia como dijiste al principio, pero que pueda usar tu sub de claude o openai almigual que el chat con ia que ya tenemos inolementado y incluir en opciones que quieres usar como motor de traduccion"
- L3 "ejecutalo"
