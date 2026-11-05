# bugs-round-3
Third round of the Notion "Bugs de Ferrocast" audit (collection 5abe6bc4-56e6-408e-a1a1-03faf5dc1ccb): the last 15 Media cards, in 3 groups, one PR each, test-first, CHANGELOG "### Fixed" entry per user-visible fix, Notion card → Corregido after merge.
Rule: if a card is already fixed (e.g. by rounds 1-2) or not reproducible, report evidence instead of changing code.

## Specs
### S1 (T1) Hotkey recorder accepts Shift+letter as a global hotkey
Card: https://app.notion.com/40e3a474adbb811ebf99cc0bcd3b8a07 · Confianza: Confirmado
- Evidencia: crates/app/src/draw.rs:1517
- Problema: ⇧A registers a system-wide hotkey, so typing a capital A in any app opens the launcher or runs the command.
- Reproducir: Settings → Record… → ⇧A → type 'A' anywhere.
- Arreglo propuesto: Require Ctrl, Alt or Cmd unless the key is an F-key.

### S2 (T1) A hotkey registration error is hidden by the 'Hotkey set to …' toast
Card: https://app.notion.com/40e3a474adbb81468356c286cd69fdbd · Confianza: Confirmado
- Evidencia: crates/app/src/draw.rs:1545-1548; app.rs:1864-1866
- Problema: If the shortcut is already taken, registration fails but the user sees a success toast and the hotkey does nothing.
- Reproducir: Give command B the same hotkey as command A.
- Arreglo propuesto: Show success only when apply_hotkeys reports no error for this spec; check duplicates against plan_hotkeys before saving.

### S3 (T1) Deleting a custom theme is immediate, permanent, no confirm or undo
Card: https://app.notion.com/40e3a474adbb817cbef4ca41fe13094d · Confianza: Confirmado
- Evidencia: crates/app/src/draw.rs:10407-10409; theme_ctl.rs:48-54
- Problema: One click destroys a hand-built theme; with sync on, the deletion reaches the other Macs.
- Reproducir: Settings → General → custom theme → Delete.
- Arreglo propuesto: Confirm or offer an Undo toast like the settings tab reset.

### S4 (T1) Force Kill Process, Clear Calculator History and Clear Shelf run with no confirmation
Card: https://app.notion.com/40e3a474adbb81d7888df6151d763230 · Confianza: Confirmado
- Evidencia: crates/app/src/app.rs:6967-6976 (destructive list omits KillProcess, ClearCalcHistory, ClearShelf) vs .danger() at 5428, 5340, 5328; Force Quit asks (6974)
- Problema: ⌘↵ on a process row sends SIGKILL at once; Clear History and Clear Shelf wipe without asking and without undo.
- Reproducir: Processes → select a row → ⌘↵.
- Arreglo propuesto: Add KillProcess(_, true), ClearCalcHistory and ClearShelf to the destructive match with translated messages.

### S5 (T1) Confirm prompts for Restart / Shut Down / Log Out / Empty Trash / Quit All Apps are never translated
Card: https://app.notion.com/40e3a474adbb81899c91fb7bd333522d · Confianza: Confirmado
- Evidencia: crates/app/src/app.rs:5576 format!("{}?", c.title()); macos/src/system.rs:159-187; draw.rs:6605
- Problema: The most dangerous prompts show in English in the 6 other languages.
- Reproducir: Language German → Root → Shut Down → 'Shut Down?'.
- Arreglo propuesto: Build the prompt with trf("{command}?") from tr(title), or add rows per command.

### S6 (T1) Custom-theme editor is untranslated and outside the i18n scan
Card: https://app.notion.com/40e3a474adbb81b8953de7755ca4506e · Confianza: Confirmado
- Evidencia: crates/app/src/draw.rs:10301-10303 (#[cfg(test)] mod tests declared before 143 production lines); i18n_scan_tests.rs:131-133 cuts the file there; draw.rs:10350 format! contrast text
- Problema: Theme name, Base, Accent, Surface, Secondary text, Separator, Theme file, Rename, descriptions and contrast warnings show in English in the 6 other languages; the scan tests still pass.
- Reproducir: cargo test i18n_scan → 3 passed; grep the literals after line 10300 against strings.tsv.
- Arreglo propuesto: Move the test mod declaration to the end of draw.rs (or make production_part skip only the mod line), add the rows, use trf for the contrast text.

### S7 (T2) Studio editor drops unsaved edits when the last save fails
Card: https://app.notion.com/40e3a474adbb81d793b5e9cd2dacec59 · Confianza: Confirmado
- Evidencia: crates/app/src/editor_panels.rs:1487-1496; app.rs:6224-6231 editor_gc; app.rs:6206-6219 open_editor
- Problema: If project.json cannot be written (read-only folder, disk full), closing or leaving the editor discards the in-memory project; the error only showed in the footer that is gone.
- Reproducir: Open a project, make its folder read-only, change a zoom, press Esc.
- Arreglo propuesto: After the flush, if autosave.pending() is set, keep the view or toast the error before dropping the editor.

### S8 (T2) A late, older translation overwrites the user's newer selection
Card: https://app.notion.com/40e3a474adbb81d195d6f115a22d3d35 · Confianza: Sospecha
- Evidencia: crates/app/src/app.rs:3815-3847 (run != translate_run never discarded; only EffectStarted checks at 3880); core translate.rs:473-479
- Problema: Translate A, then select B and translate again: if A arrives later it replaces B's selection and B is inserted too; with Paste output both get pasted.
- Reproducir: Two quick wheel translations in TextEdit with a slow provider.
- Arreglo propuesto: Deliver in place only when run == translate_run; show or copy older runs.

### S9 (T2) Every Settings change writes settings.json, rebuilds the root index and pushes tiling config each frame
Card: https://app.notion.com/40e3a474adbb81df8762c66107a94b83 · Confianza: Confirmado
- Evidencia: crates/app/src/draw.rs:8475 → app.rs:1617-1626; colour picker draw.rs:7222 → theme_ctl.rs:27-28
- Problema: Typing in a field, dragging a slider or the colour picker does synchronous disk I/O, a full item rebuild and a sync mark at frame rate on the UI thread.
- Reproducir: Drag the custom-theme colour picker and watch themes.json mtime / fs_usage.
- Arreglo propuesto: Debounce persisting (as notes do); rebuild or resend tiling only when the related fields change.

### S10 (T2) Homebrew 'Check for Updates' shows 'brew: unknown error' exactly when an update exists
Card: https://app.notion.com/40e3a474adbb816b97ede15f305903f9 · Confianza: Confirmado
- Evidencia: crates/app/src/app.rs:5841-5856; Homebrew cmd/outdated.rb:97 (exit 1 when outdated)
- Problema: brew outdated exits 1 when ferrocast is outdated → falls into the error arm with empty stderr → 'brew: unknown error'. Without brew update first, a stale tap reports up to date.
- Reproducir: Older formula installed → Check for Updates from the Cellar build.
- Arreglo propuesto: Parse stdout regardless of exit code; refresh the tap first as brew_upgrade does.

### S11 (T2) A recording project with screen.mov but no duration/sources is never recovered
Card: https://app.notion.com/40e3a474adbb8186a646fd8b70324ad9 · Confianza: Confirmado
- Evidencia: crates/app/src/record_ctl.rs:735-740 (finalize_project only on Cmd::Stop or helper failure); PR #113 stops a recording before exit but cannot repair projects left broken earlier (crash, kill -9, pre-fix builds).
- Problema: If the app died mid-recording, project.json lacks the duration and the camera/mic/system sources although screen.mov exists; the editor shows a 0-length project. Recovering needs the capture helper to probe the media (a new helper command), which is why it was left out of PR #113.
- Reproducir: kill -9 Ferrocast during a recording; open the recording.
- Arreglo propuesto: Add a capture-helper 'probe' command that reads duration/tracks from screen.mov and the source files; on opening a project with screen.mov but no duration, call it and finalize project.json.

### S12 (T3) Snippet expansion does not check the target app it claims to check
Card: https://app.notion.com/40e3a474adbb81138232e6d541cb117a · Confianza: Confirmado
- Evidencia: crates/macos/src/expansion.rs:132-137 (comment says 'don't type into a new app' but only checks frontmost().is_some())
- Problema: If focus changes between the keyword match and inject (spawned thread), N backspaces + Cmd-V go to the other app and delete its text.
- Reproducir: Type the keyword and switch apps at once.
- Arreglo propuesto: Capture the frontmost pid in the tap callback and abort inject if it differs.

### S13 (T3) Synthetic paste uses hard-coded keycode 9, wrong shortcut on non-QWERTY layouts
Card: https://app.notion.com/40e3a474adbb81ce851cce436b6d4c31 · Confianza: Sospecha
- Evidencia: crates/macos/src/keyboard.rs:11 (V = 9), 47-49
- Problema: On Dvorak that key is K, so expansion sends Cmd-K after deleting the keyword.
- Reproducir: Dvorak input source, trigger a snippet.
- Arreglo propuesto: Resolve the keycode for 'v' in the current layout (UCKeyTranslate / TIS) or paste via AX.

### S14 (T3) Now Playing pill never auto-hides after a pause unless another event arrives
Card: https://app.notion.com/40e3a474adbb8149a55fdfbb0b366aae · Confianza: Confirmado
- Evidencia: crates/core/src/nowplaying.rs:326-327 (idle rule only in refresh(), event-driven); macos/src/nowplaying.rs:347-350 (identical snapshots suppressed); app.rs:1640, 4059-4079 (no tick); nowplaying_tests.rs:166-167 feeds repeated snapshots
- Problema: Paused → snapshot stops changing → no events → the pill stays at the notch indefinitely instead of hiding after IDLE_HIDE_SECS.
- Reproducir: Play, pause, do not touch the notch.
- Arreglo propuesto: Add Ctl::tick(now) calling refresh and schedule it from the app at last_active + IDLE_HIDE_SECS.

### S15 (T3) Now-playing worker thread decodes AppKit images with no autorelease pool
Card: https://app.notion.com/40e3a474adbb8168b8a0ca9b5e19e40a · Confianza: Sospecha
- Evidencia: crates/macos/src/nowplaying.rs:277-300; nowplaying_art.rs:16-57; only language.rs:18 uses autoreleasepool
- Problema: Autoreleased objects from each cover decode accumulate until thread exit → RSS grows with every track change.
- Reproducir: Play a long playlist with the notch player on and watch RSS.
- Arreglo propuesto: Wrap each run_once iteration (or ensure) in objc2::rc::autoreleasepool.

## Tasks
- [x] T1 Confirmations, hotkeys and themes UI — S1-S6 — worker, branch fix/r3-ui, worktree ../raycast-r3-ui — commit: PR #122 merged
- [x] T2 App flows: editor save, translate, settings writes, Homebrew, recording recovery — S7-S11 — worker, branch fix/r3-flows, worktree ../raycast-r3-flows — commit: PR #123 (S7-S10) merged; S11 PR #124
- [x] T3 macOS input and Now Playing — S12-S15 — worker, branch fix/r3-macos, worktree ../raycast-r3-macos — commit: PR #121 merged

## Log
- L1 user: "sigamos"
- L2 parent: continues odd/tasks/bugs-round-2.md (closed 86a2f65). Groups split by file area so the three PRs overlap only in CHANGELOG.md, app.rs call sites and test-module hooks.
- L3 parent: S11 needed files outside T2's surfaces; parent chose the spec's helper `probe` command (AVFoundation, no ffmpeg dependency) and shipped it as its own PR #124.
- L4 parent: closed. PRs #121 (T3), #122 (T1, plus a 400 ms double-click guard on theme delete), #123 (T2 S7-S10), #124 (S11) merged; each branch re-merged with master and re-tested before merge. 15 Notion cards → Corregido. Board: Alta 7/7, Media 41/41 Corregido, Baja 62 Pendiente. Post-merge cargo test --workspace on master 1124033: 2702 passed, 0 failed.
