# bugs-round-2
Second round of the Notion "Bugs de Ferrocast" audit (collection 5abe6bc4-56e6-408e-a1a1-03faf5dc1ccb): 26 Media cards grouped by area, one PR per group, test-first, CHANGELOG "### Fixed" entry per user-visible fix, Notion card → Corregido after merge.
Rule: if a card is already fixed (e.g. by round 1) or not reproducible, report evidence instead of changing code.

## Specs
### S1 (T1) Fuzzy matcher misses valid matches by jumping ahead to a later word start
Card: https://app.notion.com/40e3a474adbb818bba3de11a205e44fb · Confianza: Confirmado
- Evidencia: crates/core/src/fuzzy.rs:95-101 (no backtracking after a word-start pick)
- Problema: fuzzy_match("ac", "xabc a z") → None although a…c is a subsequence; items can vanish from results.
- Reproducir: fuzzy_match("ac","xabc a z",Sensitivity::Low) = None.
- Arreglo propuesto: Retry with plain leftmost greedy when the word-start pass fails.

### S2 (T1) round_money overflows on a zero amount (panics in debug builds)
Card: https://app.notion.com/40e3a474adbb818f8126c98742dddbd0 · Confianza: Confirmado
- Evidencia: crates/core/src/calc.rs:459-460 (2 - log10(0).floor() as i32 = 2 - i32::MIN)
- Problema: "0 usd to eur" panics in dev/test builds; release wraps silently.
- Reproducir: evaluate_with_rates("0 usd to eur", 16.0, Some(&rates)) panics at calc.rs:460.
- Arreglo propuesto: Return early when v == 0 or !finite; saturating_sub.

### S3 (T1) Calculator reads commas between function arguments as thousands separators
Card: https://app.notion.com/40e3a474adbb81cab966efd20ed26be5 · Confianza: Confirmado
- Evidencia: crates/core/src/calc.rs:41-46
- Problema: max(1,2,3) → 12; min(5,1,2) → 2.
- Reproducir: calc::evaluate("max(1,2,3)") = "12".
- Arreglo propuesto: Comma is a separator only when exactly 3 digits follow and the next char is not a digit, and never inside function parentheses.

### S4 (T1) EPUB reader passes zip entry names to unzip, which treats them as wildcard patterns
Card: https://app.notion.com/40e3a474adbb81b19d72f2da92ebcd75 · Confianza: Confirmado
- Evidencia: crates/core/src/epub.rs:133 (unzip -p <epub> <entry>)
- Problema: ch[1].xhtml returns ch1.xhtml (wrong chapter); * dumps every file concatenated.
- Reproducir: unzip -p t.zip 'ch[1].xhtml' printed the other file.
- Arreglo propuesto: Escape [ ] * ? \ in the entry name, or read the zip in-process.

### S5 (T2) One unknown enum value in settings.json resets every setting; via sync it blocks settings sync for good
Card: https://app.notion.com/40e3a474adbb8136bcf4dd252347d4e1 · Confianza: Confirmado
- Evidencia: crates/core/src/models.rs:249-250; storage.rs:40-48 (file renamed .corrupt, defaults loaded); sync_io.rs:917-936; app/src/sync_ctl.rs:362-365 (returns silently every run)
- Problema: After a downgrade or a value from a newer build, the whole settings file is set aside; over sync every incoming settings change is skipped with no error.
- Reproducir: serde_json::from_str::<Settings>(r#"{"theme":"Sepia"}"#) → unknown variant.
- Arreglo propuesto: Per-field lenient deserialization (deserialize_with fallback) or #[serde(other)] variants.

### S6 (T2) Applying any sync domain clears the clipboard dirty flag, so a fresh copy is not synced
Card: https://app.notion.com/40e3a474adbb817a88b6dd2c73d5ad5c · Confianza: Confirmado
- Evidencia: crates/app/src/sync_ctl.rs:432 (clip_dirty false after the match for every domain); draw.rs:104-105; sync_ctl.rs:109; services.rs:634
- Problema: A copy landing after the last flush, when a snippets/notes/settings result is applied in the same drain() batch, loses its dirty flag → not published until the next copy.
- Reproducir: Sync on, window hidden, copy just before another domain's sync completes.
- Arreglo propuesto: Clear clip_dirty only in the clipboard arm (or not at all).

### S7 (T2) Clipboard entries from other Macs are appended oldest-first and are the first the 1000-entry cap drops
Card: https://app.notion.com/40e3a474adbb81a88088cc793ef73fdc · Confianza: Confirmado
- Evidencia: crates/core/src/sync.rs:316-328; sync_io.rs:952-987 clipboard_normalize (never sorts by copied_at); app.rs:4499-4535
- Problema: A minute-old entry from another Mac appears below this Mac's older entries and is cut first when the list is full.
- Reproducir: merge_as local [new@100, old@10] + remote r1@200, r2@300 → [new, old, r1, r2].
- Arreglo propuesto: Sort non-pinned entries by copied_at desc in clipboard_normalize before the cap.

### S8 (T2) Every copy writes the whole clipboard history while holding the lock the UI needs
Card: https://app.notion.com/40e3a474adbb819d8c78eeb9300fd740 · Confianza: Sospecha
- Evidencia: crates/app/src/services.rs:556, 632-633; sync_ctl.rs:143, 418-426
- Problema: Each copy serialises up to ~200 MB of pretty JSON (1000 × 200 KB) and prunes images under the mutex the clipboard view and sync also take → stutter.
- Reproducir: Large history, Clipboard view open, copy repeatedly.
- Arreglo propuesto: Clone the list, drop the lock, save/prune off-lock or debounce.

### S9 (T3) Export Data runs tar on the UI thread and archives the Node runtime, extensions and update downloads
Card: https://app.notion.com/40e3a474adbb8114baefd81c4e16e7b2 · Confianza: Confirmado
- Evidencia: crates/app/src/app.rs:6639-6651; services.rs:2538-2547; core extensions/mod.rs:19, node.rs:92
- Problema: The launcher freezes while gzip packs ~100+ MB of Node runtime, node_modules and updates/ leftovers; huge backup, no progress.
- Reproducir: Enable Raycast extensions (Node downloaded), run Export Data.
- Arreglo propuesto: Worker thread; exclude runtime, updates, node_modules, *.tmp.

### S10 (T3) Backups include extension preferences that hold secrets
Card: https://app.notion.com/40e3a474adbb81deb662da7ad00d6ad8 · Confianza: Confirmado
- Evidencia: crates/app/src/services.rs:2531-2546; core extensions/storage.rs:53-54 (preferences.json 0600 with password prefs)
- Problema: API keys and passwords typed into Raycast extension preferences (and localstorage.json) go into a Downloads archive the app says is safe to share.
- Reproducir: Save a password preference, Export Data, tar tzf.
- Arreglo propuesto: Exclude extensions/*/preferences.json and localstorage.json (or the whole extensions/ folder).

### S11 (T4) Clipboard.copy concealed flag is dropped, so secrets reach third-party clipboard managers
Card: https://app.notion.com/40e3a474adbb81fbbdb8cc0b7103e55d · Confianza: Confirmado
- Evidencia: extensions-runtime/src/api.js:148 (sends concealed); crates/app/src/ext_glue.rs:802-806 (ignored; plain write_clipboard); grep finds no reader of 'concealed' in crates
- Problema: Password/OTP extensions call Clipboard.copy(secret, {concealed:true}); Ferrocast writes a plain item without org.nspasteboard.ConcealedType, so Maccy/Paste record the secret.
- Reproducir: Code reading.
- Arreglo propuesto: When concealed, write with the ConcealedType/TransientType markers (the macos clipboard crate already detects them).

### S12 (T4) showHUD is never visible (drawn in the launcher window, which is hidden at once)
Card: https://app.notion.com/40e3a474adbb81feb151f73c66dd90f1 · Confianza: Confirmado
- Evidencia: crates/app/src/ext_glue.rs:617-621 (Hud => toast_verbatim; hide(); reset_to_root()); app.rs:2924-2927; ext_glue.rs:~440
- Problema: 'Copied to Clipboard' and every no-view showHUD('Done') never appears; toasts have a hidden-window fallback, HUDs do not.
- Reproducir: Any no-view command ending with await showHUD('x').
- Arreglo propuesto: When hiding, show the HUD text with services::notify_verbatim or a small floating panel.

### S13 (T4) Every toast property change sends a separate macOS notification while the launcher is hidden
Card: https://app.notion.com/40e3a474adbb81beb0afc21b41e203c9 · Confianza: Confirmado
- Evidencia: extensions-runtime/src/api.js:118-127 (setters each _send); ext_glue.rs:608-616
- Problema: showToast(Animated 'Loading') then style=Success, title='Done' → 3 Notification Center banners in no-view commands, including the Animated one.
- Reproducir: Code reading.
- Arreglo propuesto: Send only after show(), debounce updates, skip Animated toasts when hidden.

### S14 (T4) Action ids are tree positions, so a re-render between display and Enter can run a different (destructive) action
Card: https://app.notion.com/40e3a474adbb816faa4cfbc994517866 · Confianza: Sospecha
- Evidencia: extensions-runtime/src/reconciler.js:119-137 (_id = child-index path); main.js:125-128
- Problema: After a refetch removes item A, path '0.0.0.0.1' becomes B's Delete.
- Reproducir: Code reading; UI race not reproduced.
- Arreglo propuesto: Include a render generation in action ids and refuse stale ids, or key on List.Item id + action index.

### S15 (T4) LocalStorage does a synchronous whole-file read and rewrite on the UI thread per call; a corrupt file is silently reset
Card: https://app.notion.com/40e3a474adbb81b5838bc0e4e53d3349 · Confianza: Sospecha
- Evidencia: crates/app/src/ext_glue.rs:837 (Reply::Now on UI thread); crates/core/src/extensions/storage.rs:58-80
- Problema: N setItem calls = N full-file reads+writes on the main thread (O(N²)); an unreadable file is treated as {} and the next setItem overwrites it.
- Reproducir: for (i<5000) await LocalStorage.setItem('k'+i, big).
- Arreglo propuesto: Serve through ext_later with an in-memory per-extension cache; refuse writes when the existing file fails to parse.

### S16 (T5) /changelog/<version>/ returns 500 instead of 503 when the database is unavailable
Card: https://app.notion.com/40e3a474adbb814fb827c26733c2cfc7 · Confianza: Confirmado
- Evidencia: web/src/pages/changelog/[version].astro:12-16
- Problema: When ok is false release is null and release! reaches releaseMeta(null), which throws.
- Reproducir: No DATABASE_URL → /changelog/0.5.0/ → 500 (TypeError at releaseMeta).
- Arreglo propuesto: Render the 503 fallback before releaseMeta when !ok.

### S17 (T5) RSS and JSON feeds return 200 with no items, publicly cached, when the database is down
Card: https://app.notion.com/40e3a474adbb815caa44d45d04f3d299 · Confianza: Confirmado
- Evidencia: web/src/pages/changelog/rss.xml.ts:8,23; changelog.json.ts:8,26
- Problema: loadReleases() ?? [] turns an outage into a valid empty feed with public max-age=300, swr=3600; readers treat it as all releases removed.
- Reproducir: No DATABASE_URL → /changelog.json and /changelog/rss.xml → 200.
- Arreglo propuesto: Respond 503 no-store when loadReleases() returns null.

### S18 (T5) npm run build overwrites the tracked web/CHANGELOG.md, and the committed copy is out of date / changelog-push publishes the stale web/CHANGELOG.md instead of the root file that triggered it
Card: https://app.notion.com/40e3a474adbb81ed82ffe91077e05a12 · Confianza: Confirmado
- Evidencia: web/package.json prebuild; web/scripts/sync-changelog.mjs:10-11; cmp web/CHANGELOG.md CHANGELOG.md differs (Unreleased Added block missing) || .github/workflows/changelog-push.yml:5,12; web/src/lib/changelog/read-source.ts:5-13 (web copy preferred); push.ts:13; CONTRIBUTING.md:39
- Problema: Full-checkout builds leave the repo dirty; the Docker build (web/ only) uses the stale copy for the sitemap. || Root CHANGELOG.md edits trigger the job but push.ts reads the committed web copy (refreshed only by release.sh): Unreleased entries and fixes never reach the site between releases; the documented dry-run validates the wrong file.
- Reproducir: diff the ## headings of both files.
- Arreglo propuesto: Stop tracking web/CHANGELOG.md and copy it into the Docker context, or sync in CI with a diff check. || Run sync-changelog.mjs before push in the workflow (or --file ../CHANGELOG.md); prefer the root file in resolveChangelogPath.

### S19 (T5) Changelog parser silently drops continuation paragraphs and prose under a category
Card: https://app.notion.com/40e3a474adbb81f09ccbf8d917a7f5a3 · Confianza: Confirmado
- Evidencia: web/src/lib/changelog/parse.ts:125-130
- Problema: A blank line ends the entry; later indented or plain lines under a ### category are neither attached nor reported, so text vanishes from the website.
- Reproducir: tsx: '- A: first\n\n  continued\n\nStray prose' → bodyMd 'first'.
- Arreglo propuesto: fail() on text outside an entry, or append indented continuations.

### S20 (T5) CSP report endpoint: anyone can fill the table; per-IP rate limit bypassed with X-Forwarded-For; summary public without token
Card: https://app.notion.com/40e3a474adbb81b9a833cfc638820930 · Confianza: Confirmado
- Evidencia: web/scripts/csp-report-handle.mjs:11-27,85; csp-report-store.mjs:4,53; csp-report-auth.mjs:16
- Problema: Client IP is the first XFF value (client-controlled) so forged IPs get new buckets; the bucket Map is never pruned; any document-uri accepted; after 500 keys new keys are dropped so real violations stop being recorded; summary is public when CSP_REPORT_SUMMARY_TOKEN is unset.
- Reproducir: Local server: 125 POSTs from one IP → 120×204 then 429; 5 POSTs with XFF 10.0.0.1..5 → all 204; GET /api/csp-report/summary/ shows the injected evil.example rows.
- Arreglo propuesto: Take the IP from the last trusted XFF hop or the socket; prune/cap rateByIp; accept only document-uri on ferrocast.app; require the summary token.

### S21 (T5) Spanish docs miss content added to the English pages
Card: https://app.notion.com/40e3a474adbb81a4b534cab9dab5bfeb · Confianza: Confirmado
- Evidencia: files/drag-wheel.md:9 (mouse/chips sentence, b94aee6); launcher/tiling.md (Other options, Related, Limits missing in ES); launcher/notes.md:106 (With the mouse); launcher/keyboard-shortcuts.md (Notes window / tool windows sections); files/index.mdx (1 fewer kbd)
- Problema: Spanish readers get incomplete docs (mouse paths, tiling options, Related sections).
- Reproducir: diff heading lists EN vs ES for tiling.md, notes.md, keyboard-shortcuts.md; grep -c chip es/docs/files/drag-wheel.md → 0.
- Arreglo propuesto: Translate the missing sections; add a test comparing heading counts EN vs ES.

### S22 (T6) apps::trash runs osascript with no timeout on the main thread; the file leaves the list even on failure
Card: https://app.notion.com/40e3a474adbb8107a40fdd52614eab71 · Confianza: Confirmado
- Evidencia: crates/macos/src/apps.rs:414 → system::osascript (no deadline, system.rs:28-34); app.rs:7650, 7654
- Problema: First use shows the Finder Automation prompt; the UI freezes until osascript returns. The entry is dropped from results even when trashing failed.
- Reproducir: Reset Finder Automation in TCC, trash a file from search.
- Arreglo propuesto: NSFileManager.trashItemAtURL off the main thread; remove the entry only on Ok.

### S23 (T6) trash_app moves a running menu-bar or agent app to the Trash
Card: https://app.notion.com/40e3a474adbb81768c71c1827ea7968f · Confianza: Confirmado
- Evidencia: crates/macos/src/apps.rs:398-404 uses running::running_apps() which keeps only ActivationPolicy::Regular (running.rs:32-38)
- Problema: Accessory (LSUIElement) and Prohibited apps never count as running, so they are trashed while running despite the 'quit it first' contract.
- Reproducir: Start a menu-bar-only app from /Applications and Move to Trash it.
- Arreglo propuesto: Check NSWorkspace.runningApplications without the policy filter, or runningApplicationsWithBundleIdentifier.

### S24 (T6) raise_window makes AX calls with no messaging timeout on the UI thread
Card: https://app.notion.com/40e3a474adbb812381bef1009498f699 · Confianza: Confirmado
- Evidencia: crates/macos/src/windows.rs:493-510 (no AXUIElementSetMessagingTimeout; focused_window at 384-393 has one); app.rs:5703
- Problema: A hung target app blocks each AX call ~6 s per window, freezing the launcher.
- Reproducir: Pick an Open Window entry for an app stopped with SIGSTOP.
- Arreglo propuesto: AXUIElementSetMessagingTimeout(0.25) as tiling.rs bound() does, or run off the main thread.

### S25 (T6) raise_window matches windows by title, which is empty without Screen Recording
Card: https://app.notion.com/40e3a474adbb814b86a2cb61e35ace4e · Confianza: Sospecha
- Evidencia: crates/macos/src/running.rs:88 (kCGWindowName omitted without Screen Recording); windows.rs:500-501 (== on AX title)
- Problema: Empty or duplicate titles raise the wrong window or none.
- Reproducir: Without Screen Recording, two windows of one app; choose the second.
- Arreglo propuesto: Match by _AXUIElementGetWindow / CGWindowID (already in WindowInfo) or by frame.

### S26 (T6) Keep Awake toggle leaves a zombie and kills caffeinate processes it did not start
Card: https://app.notion.com/40e3a474adbb819b91ffdbf65df3e10b · Confianza: Confirmado
- Evidencia: crates/macos/src/system.rs:304-317 (Child dropped, never waited); 347-350 (pgrep -x caffeinate)
- Problema: Our caffeinate stays <defunct> until exit; any user caffeinate is killed and reported as ours.
- Reproducir: Python experiment: unreaped child stays Z; caffeinate -i sleep 999 in Terminal then Toggle Keep Awake kills it.
- Arreglo propuesto: Keep the Child in a static Mutex<Option<Child>>, kill() + wait(); no pgrep.

## Tasks
- [x] T1 Core correctness: fuzzy matcher, calculator, EPUB — S1-S4 — worker, branch fix/r2-core, worktree ../raycast-r2-core; surfaces: crates/core/src/{fuzzy,calc,epub}.rs and their tests, CHANGELOG.md — commit: PR #115 merged ee4d577
- [x] T2 Settings and sync resilience — S5-S8 — worker, branch fix/r2-sync, worktree ../raycast-r2-sync; surfaces: crates/core/src/{models,storage,sync,sync_io}.rs, crates/app/src/{sync_ctl,services}.rs (clipboard/sync parts), tests, CHANGELOG.md — commit: PR #120 merged (independently verified; 3 verifier findings fixed)
- [x] T3 Export Data / backups — S9-S10 — worker, branch fix/r2-backup, worktree ../raycast-r2-backup; surfaces: crates/app/src/services.rs (backup), app.rs (export flow), tests, CHANGELOG.md — commit: PR #117 merged
- [x] T4 Extensions host and runtime — S11-S15 — worker, branch fix/r2-ext, worktree ../raycast-r2-ext; surfaces: extensions-runtime/**, crates/app/src/ext_glue.rs, crates/core/src/extensions/**, clipboard write helper, tests, CHANGELOG.md — commit: PR #119 merged
- [x] T5 Website: changelog, feeds, CSP endpoint, Spanish docs — S16-S21 — worker, branch fix/r2-web, worktree ../raycast-r2-web; surfaces: web/**, .github/workflows/changelog-push.yml, CONTRIBUTING.md — commit: PR #116 merged
- [x] T6 macOS: trash, windows, Keep Awake — S22-S26 — worker, branch fix/r2-macos, worktree ../raycast-r2-macos; surfaces: crates/macos/src/{apps,windows,running,system}.rs, minimal call sites in crates/app, tests, CHANGELOG.md — commit: PR #118 merged (S22 already fixed by #110)

Deferred to round 3 (Media, Pendiente):
- Snippet expansion does not check the target app it claims to check (https://app.notion.com/40e3a474adbb81138232e6d541cb117a)
- Hotkey recorder accepts Shift+letter as a global hotkey (https://app.notion.com/40e3a474adbb811ebf99cc0bcd3b8a07)
- A hotkey registration error is hidden by the 'Hotkey set to …' toast (https://app.notion.com/40e3a474adbb81468356c286cd69fdbd)
- Now Playing pill never auto-hides after a pause unless another event arrives (https://app.notion.com/40e3a474adbb8149a55fdfbb0b366aae)
- Now-playing worker thread decodes AppKit images with no autorelease pool (https://app.notion.com/40e3a474adbb8168b8a0ca9b5e19e40a)
- Homebrew 'Check for Updates' shows 'brew: unknown error' exactly when an update exists (https://app.notion.com/40e3a474adbb816b97ede15f305903f9)
- Deleting a custom theme is immediate, permanent, no confirm or undo (https://app.notion.com/40e3a474adbb817cbef4ca41fe13094d)
- Confirm prompts for Restart / Shut Down / Log Out / Empty Trash / Quit All Apps are never translated (https://app.notion.com/40e3a474adbb81899c91fb7bd333522d)
- Custom-theme editor is untranslated and outside the i18n scan (https://app.notion.com/40e3a474adbb81b8953de7755ca4506e)
- Synthetic paste uses hard-coded keycode 9, wrong shortcut on non-QWERTY layouts (https://app.notion.com/40e3a474adbb81ce851cce436b6d4c31)
- A late, older translation overwrites the user's newer selection (https://app.notion.com/40e3a474adbb81d195d6f115a22d3d35)
- Force Kill Process, Clear Calculator History and Clear Shelf run with no confirmation (https://app.notion.com/40e3a474adbb81d7888df6151d763230)
- Studio editor drops unsaved edits when the last save fails (https://app.notion.com/40e3a474adbb81d793b5e9cd2dacec59)
- Every Settings change writes settings.json, rebuilds the root index and pushes tiling config each frame (https://app.notion.com/40e3a474adbb81df8762c66107a94b83)
- A recording project with screen.mov but no duration/sources is never recovered (https://app.notion.com/40e3a474adbb8186a646fd8b70324ad9)

## Log
- L1 user: "sidamos corriegiendo los busgs y cosas que hay en el notion"
- L2 parent: round 1 closed the 7 Alta cards (PRs #110-#114). Round 2 takes the security, data-loss and isolated-area Media cards; app UI/flows cards that all touch app.rs/draw.rs wait for round 3 to avoid merge conflicts.
- L3 parent: closed. PRs #115 (T1), #116 (T5), #117 (T3), #118 (T6), #119 (T4), #120 (T2) merged; 26 Notion cards → Corregido (S22 was already fixed by #110). T2 independently verified: clipboard save race, nested capture reset and remote-value reset fixed before merge. Notion after round 2: Alta 7/7 Corregido, Media 26 Corregido + 15 Pendiente (round 3: app UI/flows in app.rs/draw.rs), Baja 62 Pendiente. Owner actions: set CSP_REPORT_SUMMARY_TOKEN in Coolify; remove ~/.Trash/Dock.app left by a T6 worker test.
- L4 parent: post-merge cargo test --workspace on master: 2667 passed, 0 failed.
