<!-- Produced by the parent session (read-only exploration, no code changed). To be executed by Sonnet. Evidence is cited as path:line; anything not observed is marked UNVERIFIED. -->

# Translate wheel for dragged text: execution plan

User request (verbatim, L1): "traza el pan que ejecutara sonnet, la idea es que al igual que tenemos la ruleta para transformar archivos al seleccionar texto podamos traducirlo al idioma que queramos".

L2 (user, engine decision, verbatim): "vamos a dejar el translate con ia como dijiste al principio, pero que pueda usar tu sub de claude o openai almigual que el chat con ia que ya tenemos inolementado y incluir en opciones que quieres usar como motor de traduccion".

Rejected for v1 (discussed before L2): Apple's Translation framework (Swift-only, macOS 15+, needs a bundled Swift helper; app targets macOS 13, `scripts/bundle.sh:30`) and Google Translate (official API needs a paid key; the unofficial endpoint is unsupported). The Google quicklink at `storage.rs:87` stays as it is.

Goal: the same gesture that opens the file wheels (drag, tap a key, drop on a segment) works on **selected text**: drag the selection, tap the Convert key (Shift by default), a ring of target languages appears, drop on one, the translation replaces the selection (or lands on the clipboard, by setting). Local-first: the translation runs through the AI provider already configured in Settings › AI (Ollama by default, `ai.rs:3`), so nothing leaves the Mac unless the user chose a cloud provider there.

---

## 1. Current state (evidence)

| What | Where | Note |
|---|---|---|
| Wheel state machine, pure | `crates/core/src/wheel.rs:6-83` | `Sample.has_files` gates opening: a text drag is explicitly ignored (`wheel.rs:56`, test `ignores_text_drags_and_holds_from_before_the_press`, `wheel.rs:289`). `WheelKind` has Convert/Tools/Actions only. |
| Sampling thread | `crates/app/src/services.rs:280-316` | Reads `drag::drag_files()` only when a wheel key is held during a press; builds `Sample`; sends `Msg::WheelOpen(kind, files)`. |
| Drag pasteboard reader | `crates/macos/src/drag.rs:28-41` | `drag_files()` filters `NSPasteboardTypeFileURL` only. No text reader exists. |
| Wheel view | `crates/app/src/app.rs:542-549` | `View::Wheel { files: Vec<PathBuf>, center, segs, kind, sel }`. Segments built by `wheel_segments(kind, files)` (`app.rs:1491`), opened by `show_wheel` (`app.rs:1555`), applied by `apply_wheel` (`app.rs:1611`). |
| Wheel drawing | `crates/app/src/draw.rs:2014-2040, 2220` | Title and colour per kind (`CONVERT`/`TOOLS`/`ACTIONS`); hub text from `hub_text(files)` (`wheel.rs:224`); the key legend at `draw.rs:2220` lists ⇧ ⌘ ⌃ with their kinds. |
| Key switching while open | `draw.rs:1431-1438`, `wheel.rs:60-68` | Any wheel key tapped while open emits `Switch(kind)`. For a text drag, Tools/Actions have no meaning. |
| AI call | `services.rs:1262` `ai_stream(settings, history, question, on_delta)`; `ask_ai` at `services.rs:1404` streams through `Msg::AiChunk`/`Msg::AiDone`. | Providers: OpenAI-compatible (Ollama/LM Studio), Anthropic, ChatGPT, Claude Code (`models.rs:193`). System prompt is fixed (`ai.rs:8`), so translation needs its own request builder or a prompt that overrides it in the user turn. |
| Paste into the previous app | `app.rs:3302 paste_text`, `app.rs:3308 paste_after_hide` | Writes the clipboard, re-activates `prev_pid`, posts ⌘V (`keyboard.rs:48`). `show_wheel` already records `prev_pid` (`app.rs:1573`). |
| Settings for the wheel | `models.rs:236-240`, UI `draw.rs:5245-5257` | `drop_wheel`, three key names. |
| Existing wheel tests | `crates/app/src/tests.rs:401-690, 1122-1180` | Render in both themes, segment order, arrow stepping, Escape, key switching, log lines without paths. |
| Docs that describe the wheel | `README.md`, `web/src/content/docs/docs/getting-started/first-run.md`, `launcher/settings.md`, `launcher/keyboard-shortcuts.md`, `es/` mirrors | Must mention the text gesture when it ships. |
| Google Translate quicklink | `crates/core/src/storage.rs:87` | Already a default quicklink; untouched. |

**UNVERIFIED**: that macOS puts `public.utf8-plain-text` on the drag pasteboard for every app's text drag (it does for Cocoa text views and Safari; Electron and some Catalyst apps may only offer RTF/HTML). Unit 3 reads `NSPasteboardTypeString` first and falls back to RTF-to-plain via `textutil` only if the first real-session check shows the gap. Also UNVERIFIED: that the source app keeps its selection after a drag that ends over Ferrocast's overlay (needed for "replace selection"). Both are H1/H2 in the human checklist.

---

## 2. Decisions taken for this plan (change them before Sonnet starts if you disagree)

- **D1 Trigger**: drag text + tap the *Convert* key (same key as file conversions, `wheel_convert_key`). No fourth key. Tools/Actions keys during a text drag do nothing and do not switch an open Translate wheel.
- **D2 Engine**: AI only, through the same `ai_stream` the chat uses, so every provider the chat supports works for translation: OpenAI-compatible (Ollama, LM Studio, OpenAI, OpenRouter), Anthropic API key, Sign in with ChatGPT (subscription), Claude Code (subscription). New setting `translate_provider: Option<AiProvider>`: `None` = "Same as chat" (default), `Some(p)` = a fixed provider that may differ from the chat one. Model, base URL and credentials are the per-provider ones already stored (`ai_model`, `anthropic_model`, `chatgpt_model`, Keychain per provider `services.rs:736-756`), so the translation engine needs no second set of fields. Provider not configured or unreachable → toast with the same message the chat shows for that provider ("Claude Code is not installed…", missing key, not signed in). Claude Code runs as a one-shot `claude -p` (`services.rs:1283`), not the warm chat session (`app.rs:1192`): simpler, and its boot time (a few seconds) is acceptable for a drop gesture; shown by the "Translating…" toast.
- **D3 Result delivery**: setting `translate_output` with two values: `Replace` (default; paste over the selection via `paste_text`) and `Copy` (clipboard only + toast "Translated to Spanish, copied"). Either way the clipboard holds the translation.
- **D4 Languages on the ring**: setting `translate_languages: Vec<String>` of language *names*, default `["English","Spanish","French","German","Portuguese","Italian","Japanese","Chinese"]` (8 segments → 11 pt labels, `wheel.rs:187`). Source language is always auto-detected by the model. Max 12 segments (`wheel_segments` already takes 12 for Tools).
- **D5 Privacy in logs**: instance log lines name the kind, segment count and target language, never the text (same rule as "no paths", `tests.rs:920`).
- **D6 Hub text**: `"N words"` / first ~24 characters with an ellipsis.

---

## 3. Units (in order; each one commits on its own)

Common rules (same as `tangerine-parity-plan.md` §2): RED test first and record the exit code (101), then implement; closing checks run as three separate commands, each exit recorded: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. No process spawn on the UI thread. Commit tag `TW<n>: <what>`.

### TW1. Core: text drags open a Translate wheel — S, model-verifiable
Files: `crates/core/src/wheel.rs`.
- Add `WheelKind::Translate`.
- `Sample` gains `has_text: bool` (default false; `has_files` keeps priority when both are set).
- `Tracker::step`: when `left_down`, the drag pasteboard changed since the press, `!has_files && has_text` and the convert key is freshly tapped → `Open(Translate)`. While a Translate wheel is open, tools/actions taps emit nothing; the convert key tap emits nothing either (no switch target). Release emits `Release` as today.
- RED tests (write before the change): `text_drag_plus_convert_key_opens_translate`, `text_drag_ignores_tools_and_actions_keys`, `open_translate_wheel_never_switches`. Update `ignores_text_drags_and_holds_from_before_the_press` (`wheel.rs:289`): the text-drag assertion must now expect `Open(Translate)` only when `has_text` is true; with `has_text: false` (old samples) it still returns `None`, which pins the previous behaviour.
- Keep every existing wheel test green (struct update syntax `..Default::default()` already used at `wheel.rs:262`).

### TW2. Core: translation module — S, model-verifiable
New file `crates/core/src/translate.rs`, registered in `lib.rs`.
- `pub const DEFAULT_LANGUAGES: [&str; 8]`.
- `pub fn languages(setting: &[String]) -> Vec<String>`: trims, drops blanks and duplicates (case-insensitive), caps at 12, falls back to defaults when empty.
- `pub fn prompt(text: &str, target: &str) -> String`: one user-turn prompt: "Translate the following text to {target}. Reply with the translation only, no quotes, no notes, keep line breaks and formatting." followed by the text. Also `pub const SYSTEM: &str` for a dedicated system prompt; TW4 decides whether `ai_stream` can take it (see note there).
- `pub fn clean_reply(s: &str) -> String`: trims, strips one pair of wrapping quotes or a ```` ``` ```` fence, strips a leading `Translation:` label.
- `pub fn hub_text(text: &str) -> (String, String)`: `("N words", "<first 24 chars…>")` with single-line collapsing of whitespace.
- `pub fn engine_settings(s: &Settings) -> Settings`: clone of the settings with `ai_provider` replaced by `translate_provider` when it is `Some`; identity when `None`. This is the only place that decides which provider translates.
- `pub fn engine_label(s: &Settings) -> String`: "Same as chat (Ollama)" / "ChatGPT" / … for the hub and the toast.
- Tests: defaults are 8; dedupe/cap; prompt contains target and text verbatim; `clean_reply` on quoted, fenced, labelled and clean inputs; hub text on empty, short, multi-line text; `engine_settings` is identity for `None` and overrides only `ai_provider` for `Some` (every other field byte-equal).

### TW3. macOS: read text from the drag pasteboard — S, smoke only
File `crates/macos/src/drag.rs`.
- `pub fn drag_text() -> Option<String>`: first `pasteboardItems` item with `NSPasteboardTypeString` (`objc2_app_kit::NSPasteboardTypeString`), non-blank after trim, capped at 20 000 chars (longer → `None` and a toast in TW4: "Selection too long to translate").
- Test: same style as `samples_without_a_drag` (`drag.rs:43`): calling it without a drag does not panic and returns `None` or a string. No stronger claim is possible in CI.
- **UNVERIFIED** fallback (only if H1 fails): read `NSPasteboardTypeRTF` and convert with `textutil -convert txt -stdin -stdout` off the UI thread.

### TW4. App: wire the Translate wheel — M, model-verifiable + screenshot
Files: `crates/app/src/services.rs`, `app.rs`, `draw.rs`, `crates/core/src/models.rs`.
1. `services.rs:280-316`: when the convert key is held and `drag_files()` is empty, call `drag::drag_text()`; set `has_text`. New `Msg::WheelOpenText(String)`; `WheelOpen` stays for files.
2. `View::Wheel`: replace `files: Vec<PathBuf>` by `subject: WheelSubject` with `enum WheelSubject { Files(Vec<PathBuf>), Text(String) }` and `fn files(&self) -> &[PathBuf]` (empty for Text) so existing match arms change minimally. `show_wheel`, `switch_wheel`, `release_wheel`, `apply_wheel`, `wheel_segments` take the subject. `switch_wheel` is a no-op for `Text` (D1).
3. `wheel_segments(Translate, Text)`: one `WheelSeg` per `translate::languages(&settings.translate_languages)`, symbol `sf:globe` (or `sf:character.book.closed` — pick whichever `draw.rs` already resolves; **UNVERIFIED** which SF names the icon loader supports, check the existing symbol list before choosing), `pick: WheelPick::Translate(lang)`.
4. `apply_wheel` for `WheelPick::Translate(lang)`: toast "Translating to {lang} with {engine_label}…", then `services::translate_text(translate::engine_settings(&settings), text, lang, run, bus)` which spawns a thread, calls `ai_stream` with `translate::prompt`, accumulates deltas, and sends `Msg::Translated { run, lang, result: Result<String, String> }`. On `Ok` → `translate::clean_reply`, then by `translate_output`: `Replace` → `paste_text` (re-activates `prev_pid`); `Copy` → `copy_text` + toast. On `Err` → toast with the provider error; the clipboard is left untouched. No provider configured/reachable → the D2 toast. Note: `ai_stream` injects the fixed `SYSTEM_PROMPT` (`ai.rs:8`); if a translation-specific system prompt is wanted, add an optional `system: Option<&str>` to `ai_stream_with_key` and the four `build_*` request builders, defaulting to the current prompt so AI chat is unchanged (one regression test per builder: without `system`, output is byte-identical to today).
5. `draw.rs:2036`: `WheelKind::Translate => ("TRANSLATE", <a fourth colour, e.g. [180.0, 120.0, 255.0]>)`. Hub text from `translate::hub_text` for `Text`. The key legend at `draw.rs:2220` is hidden for the Translate wheel (no switch targets).
6. `models.rs`: `translate_languages: Vec<String>` (default = `DEFAULT_LANGUAGES`), `translate_output: TranslateOutput { Replace, Copy }` (default Replace), `translate_provider: Option<AiProvider>` (default `None`). `Settings` is already `#[serde(default)]` (`models.rs:206`), so existing settings files load.
7. Log lines: extend `wheel_log_line`/`wheel_pick_log_line` (`app.rs:4800-4809`) so a Translate wheel logs `kind=Translate segs=8 subject=text` and the pick logs the language, never the text. Extend the test at `tests.rs:920`.
8. Tests (`crates/app/src/tests.rs`, RED first):
   - `translate_wheel_lists_the_configured_languages_in_order` (default 8; a custom list of 3; a 15-item list capped at 12).
   - `translate_wheel_renders_in_both_themes` (reuse the `wheel_renders_for_any_segment_count_in_both_themes` harness, `tests.rs:401`).
   - `tools_or_actions_key_does_not_switch_a_translate_wheel`.
   - `picking_a_language_sends_one_translate_request_and_pastes_the_cleaned_reply`: drive `Msg::Translated` with `Ok("\"Hola\"")` and assert the clipboard write and the `Replace`/`Copy` branch. If `paste_text` cannot be asserted without a real app, assert the clipboard content and that the view is hidden; mark the ⌘V post as human (H2).
   - `translate_error_toasts_and_leaves_the_clipboard_alone`.
   - `translate_uses_the_translate_provider_not_the_chat_one`: chat = OpenAI-compatible, translate = Anthropic → the request built for the translation is an Anthropic Messages body (assert through `engine_settings` plus the existing request-builder tests; no network).
   - Existing file-wheel tests stay green with `WheelSubject::Files`.
   - Screenshot proof: `target/translate-proof/wheel.png` through the existing render harness.

### TW5. Settings UI — S, screenshot
General tab (`crates/app/src/draw.rs:5245`, after the wheel keys):
- Row "Translate wheel languages": one text field, comma-separated, saved through `translate::languages`; helper text "Drag selected text and tap the Convert key. Up to 12 languages."
- Row "After translating": segmented `Replace selection` / `Copy only`.

AI tab (`draw.rs:5307-5315`, under the chat provider segmented control):
- Row "Translation engine": segmented `Same as chat` / `OpenAI API` / `ChatGPT` / `Anthropic` / `Claude Code` bound to `translate_provider`. Under it, the same readiness hint the chat provider shows (key present, signed in, `claude` found: `draw.rs:5183`, `services.rs:1187-1215`), computed for the selected translation provider, so the user sees before dragging that the engine is usable. Choosing a provider here never changes the chat provider or its model.
- Test: settings round-trip (`Settings` serde default test exists? **UNVERIFIED**; add one that an old JSON without the new keys loads with defaults).

### TW6. Docs — S
- `README.md` and `README.es.md`: one line in the wheel section.
- `web/src/content/docs/docs/getting-started/first-run.md`, `launcher/settings.md`, `launcher/keyboard-shortcuts.md` and their `es/` mirrors: the text gesture, the three settings (languages, after-translating, translation engine), that the engine can be any chat provider including the ChatGPT and Claude subscriptions, and the privacy note (the selected text goes to the chosen engine; Ollama keeps it local).
- `CHANGELOG.md` › Unreleased › Added.

Optional, not in v1 (needs a product decision): a keyboard route "Translate Selection" command (would need a selected-text reader via ⌘C simulation, which no code provides today), and a "Detect language" hub line.

---

## 4. Execution order and sizing

TW1 → TW2 → TW3 → TW4 → TW5 → TW6. TW1-TW3 are independent and can be one commit each in under an hour; TW4 is the only medium unit (~300-400 changed lines because of the `View::Wheel` subject refactor). Total estimate: ~700 authored lines including tests and docs, six commits on a feature branch `feat/translate-wheel`.

Risk line for the whole feature: **item 3** (`View::Wheel`'s `files` field and the `Tracker` sample are shared by every existing wheel path; TW1 and TW4 carry regression tests for the file wheels) and **item 6 partially** (the real drop gesture and ⌘V paste are only provable in a session: H1-H3). No data, security, concurrency or delivery item.

---

## 5. Sonnet guardrails

- Do not add a fourth wheel key or touch `wheel_tools_key`/`wheel_actions_key` behaviour.
- No engine other than the AI providers: no Swift helper, no Google endpoint, no new credentials. The translation provider reuses the chat provider's stored model and credentials; do not add per-engine model or key fields.
- Do not change `SYSTEM_PROMPT` or the AI chat request bodies; add an optional parameter with the old behaviour as default and pin it with tests.
- Never log, toast or screenshot the dragged text; tests use short fixed strings.
- Do not spawn `ai_stream` on the UI thread; use the thread + `Msg` pattern of `ask_ai` (`services.rs:1404`).
- Keep `drag_files()` precedence: a drag that carries both file URLs and a string is a file drag.
- Do not implement RTF fallback or a "Translate Selection" command unless H1 fails or the user asks.
- Record each check's exit code separately; never pipe `cargo test` into `tail` without `set -o pipefail`.

## 6. Human checklist (real session, cannot be proven by tests)

- H1 Drag selected text from TextEdit, Safari, Notes, VS Code and Slack; tap Shift; the Translate wheel opens with the hub showing the word count. Note which apps expose plain text on the drag pasteboard.
- H2 Drop on "Spanish" with `Replace selection`: the source app regains focus and the selection is replaced by the translation; nothing was dropped into the source app.
- H3 Same with `Copy only`: the selection is untouched and the clipboard holds the translation.
- H4 Ollama stopped → the provider error toast appears, clipboard unchanged.
- H4b Translation engine = ChatGPT while chat = Ollama: the translation comes back from ChatGPT (visible in the toast label) and the chat still answers through Ollama. Same with Claude Code (first translation pays its boot time).
- H5 Dragging a file still opens the Convert/Tools/Actions wheels exactly as before; tapping Shift while dragging text with no provider configured shows the D2 toast.
- H6 A selection over 20 000 characters shows "Selection too long to translate".
