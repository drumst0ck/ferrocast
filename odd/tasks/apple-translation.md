# Apple Translation engine (offline Translate)
Plan: odd/plans/apple-translation-plan.md. Branch feat/apple-translation (worktree /tmp/fc-apple).
Deviation from the plan: no `TranslateEngine` enum and no migration. An additive `translate_apple: bool` setting is
true when the Apple engine is chosen; `translate_provider` and `translate_model` keep their meaning. Same user behaviour, no contract change.

## Specs
S1 Settings → Translate → Engine shows an **Apple** segment only when macOS ≥ 26 and the helper exists; selecting it hides the Model row and shows the Languages card and the **Open Language settings** button.
S2 Each wheel language shows one chip: `Installed`, `Needs download`, or `Not supported by Apple`; `Checking…` until the helper answers. Statuses are computed off the UI thread.
S3 With Apple selected and the pack installed, dropping text on a language delivers the translation through the same outputs as the AI engines, with no network and no AI provider configured.
S4 With the pack missing, the toast is exactly: `{Language} is not downloaded for Apple Translation. Download it in System Settings → General → Language & Region → Translation Languages.`
S5 With a language Apple does not support, the toast is exactly: `Apple Translation does not support {Language}. Pick another language or an AI engine.`
S6 A wheel language whose name has no ISO code in `language_code` behaves as S5.
S7 Existing `settings.json` files load unchanged (translate_apple defaults to false).
S8 On macOS < 26 (or helper missing) a stored Apple choice translates with the chat provider and toasts once per session: `Apple Translation needs macOS 26; using the chat provider.`
S9 The log line for an Apple translation is `translate {lang} via Apple ok chars=N ms=M`.
S10 `scripts/bundle.sh` builds and signs the helper when the SDK is ≥ 26, skips it with a NOTE otherwise, and the app still runs; CI asserts the helper is present.
S11 The helper obeys the JSON protocol, exit codes 0/1/2, 60 s timeout on the Rust side, text on stdin only.
S12 Docs (EN/ES), settings docs, settings index and CHANGELOG describe the engine and the download steps.

## Tasks
- [x] T1 S7 S9 core: `translate_apple` field, engine_label, Apple code mapping (2 commits)
- [x] T2 S11 helper source + core protocol + macOS runner (1 commit; helper exercised by hand: status, translate, unsupported, empty, bad JSON)
- [ ] T3 S3 S4 S5 S6 S8 translate_text Apple branch + outcome mapping + fallback
- [ ] T4 S1 S2 Settings tab UI + statuses + settings index
- [ ] T5 S10 bundle.sh, CI/release runner, formula caveat (Risk: item 5, own commit)
- [ ] T6 S12 docs EN/ES, CHANGELOG

## Slice boundary
After T2 the branch holds ~690 authored lines (about 260 of them this feature document and the plan). T3 onward continues on the same branch; T5 gets its own commit (Risk: item 5). Push, PR and merge stay the user's decision.

## Log
L1 (user): "realiza el plan" (odd/plans/apple-translation-plan.md)
