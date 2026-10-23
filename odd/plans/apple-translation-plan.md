# Apple Translation as an offline engine for Translate

Status: plan, not started. Written 2026-10-22 after a read-only review of the code and a spike
against the installed macOS SDK (see "Spike results"). Execute with the ODD workflow: create
`odd/tasks/apple-translation.md` from the Tasks section below before the first write.

## Goal

Add Apple's on-device Translation framework as a translation engine next to the AI providers, so
dragged text can be translated with no model running and no network, once the user has downloaded
the language packs in System Settings. Guide the user to that download from the app.

## Non-goals

- Replacing or changing the AI engines, the wheel, the outputs (show / paste / replace / copy), or
  the translating effect. Only the engine behind `translate_text` changes.
- Downloading language packs from inside the app. `TranslationSession.prepareTranslation()` shows
  a system sheet that needs a host window; this plan opens System Settings instead. Revisit later.
- Supporting Apple Translation below macOS 26. The headless API
  (`TranslationSession(installedSource:target:)`) exists only from macOS 26; earlier versions have
  a SwiftUI-only API. The app keeps `LSMinimumSystemVersion` 13.0 and hides the engine there.
- Bergamot/Marian or LibreTranslate engines.

## How it will work (user-facing)

1. Settings → Translate → Engine gets a new segment **Apple** (on-device), next to Same as chat /
   OpenAI API / ChatGPT / Anthropic / Claude Code. The Model row is hidden for Apple.
2. Under the engine, when Apple is selected, a **Languages** card lists every language on the wheel
   with a chip: **Installed**, **Needs download**, or **Not supported by Apple**. A button
   **Open Language settings** opens System Settings → General → Language & Region, where the
   "Translation Languages" section lives. A note explains the download happens once per language
   and that translations then run offline.
3. Dragging text and dropping it on a language: if the pack is installed, the text is translated
   on the Mac and delivered exactly as today. If the pack is missing, the toast says
   "German is not downloaded for Apple Translation. Download it in System Settings → General →
   Language & Region → Translation Languages." and the Settings → Translate tab is one click away.
   If Apple does not support the language, the toast says so and suggests an AI engine.
4. On macOS older than 26, or when the helper binary is missing from the bundle (see Build), the
   Apple segment is not shown and a stored `Apple` choice falls back to "Same as chat" at run time
   with a one-time toast.

## Architecture

### Why a Swift helper

The Translation framework is Swift-only (no Objective-C surface), so `objc2` cannot call it like
Vision or NaturalLanguage. The lowest-risk bridge is a tiny Swift command-line helper shipped inside
the bundle and spawned per request, the same way `services.rs` already spawns `osascript`, `ffmpeg`
and `claude`. No Swift linked into the Rust binary, no `build.rs`, no ABI concerns.

### Helper: `ferrocast-translate`

- Source: `crates/macos/swift/translate/main.swift` (single file, ~100 lines, `@main` async).
- Built by `scripts/bundle.sh` into `Contents/MacOS/ferrocast-translate`, signed with the same
  identity and flags as the app, before the app bundle is signed.
- Built with `-target arm64-apple-macos13.0 -parse-as-library -O` and
  `-Xlinker -weak_framework -Xlinker Translation`, so the binary loads on macOS 13 and answers
  `{"error":"unsupported-os"}` there; every framework use sits behind `if #available(macOS 26, *)`.
- Protocol: one JSON object on stdin, one JSON object on stdout, exit code 0 on success and 1 on
  error (2 for unsupported OS). Text travels on stdin, never in argv.

  ```json
  {"cmd":"status","source":"en","target":"de"}
  → {"status":"installed"} | {"status":"supported"} | {"status":"unsupported"}

  {"cmd":"status","target":"de"}            // source omitted: checked against the system language
  {"cmd":"translate","source":"en","target":"de","text":"Good morning"}
  → {"text":"Guten Morgen"} | {"error":"needs-download","language":"de"} | {"error":"unsupported","language":"xx"} | {"error":"<message>"}
  ```

- `source` may be omitted for `translate`: the helper then detects it with `NLLanguageRecognizer`
  (same framework the Rust side already uses). The Rust side still passes the code it detected when
  confidence is at least `MIN_CONFIDENCE` so both sides agree.
- `translate` first calls `LanguageAvailability.status(from:to:)` and maps `.supported` to
  `needs-download` and `.unsupported` to `unsupported` before creating the session, so the error
  reaches the user in words instead of a framework exception.
- The Rust side runs it with a 60 s timeout (reuse the pattern of `osascript_timeout`) and kills it
  on timeout.

### Rust side

- `crates/macos/src/apple_translate.rs` (new): `helper_path()` (next to `current_exe`, resolved
  through `instance::resolve_exe` like the update code), `available() -> bool` (macOS ≥ 26 and the
  helper exists and is executable), `status(source: Option<&str>, target: &str) -> Status`,
  `translate(source: Option<&str>, target: &str, text: &str) -> Result<String, TranslateError>`.
  `TranslateError { NeedsDownload(String), Unsupported(String), Other(String) }`. Pure JSON
  encode/decode of the protocol lives in `ferrocast_core::apple_translate` so it is unit-testable
  without macOS.
- macOS version: add `ferrocast_macos::system::os_at_least(major: u32) -> bool` reading
  `NSProcessInfo.operatingSystemVersion` through `objc2_foundation` if no helper exists yet.
- `crates/core/src/models.rs`: new enum

  ```rust
  #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
  pub enum TranslateEngine { #[default] Chat, Apple, Ai(AiProvider) }
  ```

  and a new field `translate_engine: TranslateEngine` on `Settings` (the struct is
  `#[serde(default)]`, so old files load). Keep `translate_provider` for one release as the
  migration source: in `Settings::load` (or wherever post-deserialize fix-ups run; add one if
  missing), when `translate_engine == Chat && translate_provider.is_some()`, set
  `translate_engine = Ai(provider)`. Writing continues to mirror `translate_provider` from
  `translate_engine` so a downgrade keeps the choice. `translate_model` keeps its meaning for `Ai`.
- `crates/core/src/translate.rs`: `engine_settings`, `engine_label`, `provider_name` read
  `translate_engine`. Add `apple_target_code(lang_name) -> Option<&str>` on top of the existing
  `language_code` table (it already maps English, native and Spanish names to ISO codes; extend it
  if a default wheel language is missing). Add `apple_outcome(result: Result<String, TranslateError>,
  lang, output) -> Outcome` mapping `NeedsDownload`/`Unsupported` to `Outcome::Fail` with the exact
  messages in "How it will work", step 3. The AI path through `run_translation` is untouched.
- `crates/app/src/services.rs::translate_text`: branch on the engine. For `Apple`: detect the
  source with `ferrocast_macos::language::detect`, resolve the target code, call
  `apple_translate::translate`, send the same `Msg::Translated` with the same `probe_target` logic.
  No prompt, no retry, no `parse_reply`. Log line engine label: `Apple`.
- `crates/app/src/draw.rs` Translate tab: add the `Apple` segment only when
  `apple_translate::available()` (cache the check per Settings open, it is a stat + version read);
  hide the Model row for Apple; add the Languages card and the Open Language settings button
  (`open x-apple.systempreferences:com.apple.Localization-Settings.extension`, verified bundle id
  of the pane on macOS 26/27). Statuses are computed off the UI thread (one helper call per wheel
  language, source = system language) and delivered through the existing `Bus` with a new
  `Msg::AppleLanguageStatus(Vec<(String, Status)>)`; show "Checking…" meanwhile. Recompute when the
  languages field changes or the tab is reopened.
- Run-time fallback: in `app.rs` where the translation starts, if `translate_engine == Apple` and
  `!available()`, run as `Chat` and toast once per session "Apple Translation needs macOS 26;
  using the chat provider."

### Build, signing, CI

- `scripts/bundle.sh`: after `cargo build`, if `xcrun --show-sdk-version` reports ≥ 26, compile the
  helper with `xcrun swiftc` into `dist/Ferrocast.app/Contents/MacOS/ferrocast-translate`, `chmod
  755`, and `codesign` it with the same `flags` array (identifier
  `dev.ferrocast.launcher.translate`) before signing the app. If the SDK is older, print
  "NOTE: Translation SDK not available; Apple engine will be hidden" and continue. This keeps the
  Homebrew formula (which builds on the user's machine) working on any Xcode.
- `Formula/ferrocast.rb`: no new dependency (swiftc ships with Command Line Tools); add a caveat
  line that the Apple engine needs macOS 26 and Xcode/CLT 26 at build time.
- `.github/workflows/ci.yml` and `release.yml`: move `runs-on` from `macos-14` to `macos-26` (or
  `macos-15` plus `sudo xcode-select -s /Applications/Xcode_26.x.app`), otherwise the helper is
  silently skipped and the DMG ships without the engine. Add a CI step asserting the helper is in
  the bundle: `test -x dist/Ferrocast.app/Contents/MacOS/ferrocast-translate`. Check Rust 1.95 and
  dmgbuild still run on that image.
- `scripts/notarize.sh`: nothing to change if the helper is signed with hardened runtime and a
  timestamp through the shared `flags`; verify with `codesign --verify --strict --deep`.

### Docs and copy

- `web/src/content/docs/docs/launcher/translate.md` and the `es/` twin: new subsection under
  "Engine and model" ("Apple Translation (offline)") with the download steps, the macOS 26
  requirement, and a line in "Privacy" (nothing leaves the Mac). Update "When it does not work"
  with the two new toasts.
- `web/src/content/docs/docs/reference/settings.md` (EN/ES): the Engine row and the Languages card.
- `CHANGELOG.md` Unreleased: "Translate: Apple Translation engine (on-device, offline once the
  language packs are downloaded; macOS 26)".
- Settings-index entries in `crates/core/src/settings_index.rs` for the new rows, so Settings
  search finds "Apple", "offline", "download languages".

## Specs

- S1 Settings → Translate → Engine shows an **Apple** segment only when macOS ≥ 26 and the helper
  exists; selecting it hides the Model row and shows the Languages card and the **Open Language
  settings** button.
- S2 Each wheel language shows one chip: `Installed`, `Needs download`, or `Not supported by
  Apple`; `Checking…` until the helper answers. Statuses are computed off the UI thread.
- S3 With Apple selected and the pack installed, dropping text on a language delivers the
  translation through the same outputs as the AI engines, with no network and no AI provider
  configured.
- S4 With the pack missing, the toast is exactly:
  `{Language} is not downloaded for Apple Translation. Download it in System Settings → General → Language & Region → Translation Languages.`
- S5 With a language Apple does not support, the toast is exactly:
  `Apple Translation does not support {Language}. Pick another language or an AI engine.`
- S6 A wheel language whose name has no ISO code in `language_code` behaves as S5.
- S7 Existing `settings.json` files load unchanged: `translate_provider: null` → `Chat`,
  `translate_provider: "Anthropic"` → `Ai(Anthropic)` with `translate_model` kept.
- S8 On macOS < 26 (or helper missing) a stored `Apple` engine translates with the chat provider
  and toasts once per session: `Apple Translation needs macOS 26; using the chat provider.`
- S9 The log line for an Apple translation is `translate {lang} via Apple ok chars=N ms=M` (same
  shape as today, engine label `Apple`).
- S10 `scripts/bundle.sh` builds and signs the helper when the SDK is ≥ 26, skips it with a NOTE
  otherwise, and the app still runs; CI asserts the helper is present.
- S11 The helper obeys the JSON protocol above, exit codes 0/1/2, 60 s timeout on the Rust side,
  text on stdin only.
- S12 Docs (EN/ES), settings docs, settings index and CHANGELOG describe the engine and the
  download steps.

## Tasks (work units, ~1 commit each)

| ID | Specs | Work | Route |
| --- | --- | --- | --- |
| T1 | S7 | `TranslateEngine` enum, `translate_engine` field, load-time migration, `engine_settings`/`engine_label`/`provider_name` updated; tests for both legacy JSON shapes and round-trip. | inline, test-first |
| T2 | S11 | Swift helper source + `ferrocast_core::apple_translate` protocol encode/decode with unit tests; `ferrocast_macos::apple_translate` runner with timeout; `os_at_least`. One `#[ignore]` integration test that needs macOS 26 and an installed en→es pack. | inline, test-first (core part) |
| T3 | S3 S4 S5 S6 S9 | `translate_text` Apple branch, `apple_outcome`, log label, run-time fallback (S8). Tests for the outcome mapping and the fallback decision. | inline, test-first |
| T4 | S1 S2 S8 | Settings tab: segment, Languages card, Open Language settings, `Msg::AppleLanguageStatus`, settings-index entries. Snapshot/render tests in `crates/app/src/tests.rs` for available vs unavailable. | inline |
| T5 | S10 | `bundle.sh` helper build + sign, CI/release runner change, helper presence assertion, formula caveat. Verify `scripts/tests/release_dry_run.sh` still passes. | inline; **Risk: item 5 (delivery/CI)** — own commit |
| T6 | S12 | Docs EN/ES, settings docs, CHANGELOG. | inline |

Verify at the end: run the examples in S3–S5 on this Mac (macOS 27, en/es/de/ja packs installed)
and once with a language whose pack is removed in System Settings; `cargo test --workspace`;
`scripts/bundle.sh` with and without the helper; open the DMG build and translate offline with
Wi-Fi off.

## Spike results (2026-10-22, this Mac: macOS 27.0, CLT Swift 6.4, SDK macOS 26)

- `TranslationSession(installedSource:target:)` + `translate(_:)` works headless from a CLI with
  no window and no entitlement; binary is 57 KB and links only `Translation.framework`.
- `LanguageAvailability.status(from:to:)` returned `installed` for en→es, en→de, en→ja, es→fr and
  `unsupported` for en→sw and en→xx. A `supported`-but-not-installed pair was not reproduced here
  (all tested packs were present); T2's integration test must cover it by removing a pack.
- First translation en→de took ~3.2 s wall (model load); later ones are sub-second. Keep the
  existing "Translating to … with Apple…" toast so the wait is visible.
- The Language & Region pane bundle id is `com.apple.Localization-Settings.extension`
  (`/System/Library/ExtensionKit/Extensions/Localization.appex`).

## Decisions to confirm before T1

1. Default engine stays **Same as chat**; Apple is opt-in. (Alternative: auto-pick Apple when
   available and the pack is installed, else fall back to the chat provider. More magic, harder to
   explain in a toast.)
2. Language pack download stays in System Settings (button + instructions), not in-app.
3. CI/release move to the `macos-26` GitHub runner. If that image is unavailable or breaks the
   Rust 1.95 / dmgbuild steps, the fallback is `macos-15` with Xcode 26 selected.

## Risks

- Item 5 (delivery): runner image change and a second signed binary in the bundle; notarization
  must be re-run on the first release. T5 gets its own commit and native review.
- Item 3 (contracts): `settings.json` gains a field and keeps the old one mirrored; S7 pins both
  directions.
- Unknown: whether Apple's `supported` status ever appears for a language the user never opened
  in System Settings versus `unsupported`; the integration test in T2 settles it.
