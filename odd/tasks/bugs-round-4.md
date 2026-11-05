# bugs-round-4
Fourth round of the Notion "Bugs de Ferrocast" audit (collection 5abe6bc4-56e6-408e-a1a1-03faf5dc1ccb): all 62 Baja cards, in 6 groups by area, one PR each, test-first, CHANGELOG "### Fixed" entry per user-visible fix, Notion card → Corregido after merge.
Rule: if a card is already fixed (rounds 1-3, PRs #110-#124) or not reproducible, report evidence instead of changing code. Sospecha cards are confirmed before fixing.

## Specs
### S1 (T1) docker-compose exposes Postgres on 0.0.0.0:5432 with password ferrocast (dev); og:locale on docs is en/es not en\_US/es\_ES
Card: https://app.notion.com/p/40e3a474adbb8121a4a0fc9371443d26 · Confianza: Confirmado
- Evidencia: web/docker-compose.yml; docs og:locale
- Problema: Dev-only exposure; non-standard og:locale values.
- Reproducir: Code reading.
- Arreglo propuesto: Bind to 127.0.0.1; use en\_US/es\_ES.

### S2 (T1) 15 indexable non-docs pages have descriptions over 155 characters and tests pin them
Card: https://app.notion.com/p/40e3a474adbb810e864af2692dfea932 · Confianza: Confirmado
- Evidencia: /es/difuminar-video-mac/ 170, /es/convertidor-de-archivos-gratis-mac/ 162, /clipboard-manager-mac/ 160, /ferrocast-vs-raycast/ 160, /text-expander-mac/ 160 …; blur-video.test.ts:41 pins 170; clipboard-manager.test.ts:9-10, launcher-compare.test.ts:11-12 allow 140-160; content.config.ts:14 enforces 120-155 for docs only
- Problema: Truncated in search results; tests lock the overlong values in.
- Reproducir: SEO script on dist lists 15 pages outside 120-155.
- Arreglo propuesto: One shared 120-155 test; shorten the 15.

### S3 (T1) Unknown changelog version returns an empty 404 with a meaningless Location header
Card: https://app.notion.com/p/40e3a474adbb8111b4f1f9116b4b7ed7 · Confianza: Confirmado
- Evidencia: web/src/pages/changelog/\[version\].astro:14
- Problema: Browsers ignore Location on 404 → blank page.
- Reproducir: Code reading (needs a live DB).
- Arreglo propuesto: Astro.redirect('/changelog/', 301) or rewrite to /404.

### S4 (T1) Changelog import logic never runs in the default npm test
Card: https://app.notion.com/p/40e3a474adbb81158966dfc26c446bd9 · Confianza: Confirmado
- Evidencia: web/src/lib/changelog/import.test.ts:14 describe.skipIf(!url)
- Problema: Delete-all branch, tag sync and transaction untested without DATABASE\_URL (the 3 skipped tests).
- Reproducir: npm test → 237 passed \| 3 skipped.
- Arreglo propuesto: Run against a Postgres service in CI, or unit-test with a fake transaction.

### S5 (T1) Sitemap lastmod for /tools/\<slug>/ ignores the docs MDX the page body comes from
Card: https://app.notion.com/p/40e3a474adbb815da025c6ee3eab8c47 · Confianza: Confirmado
- Evidencia: web/src/lib/sitemap/lastmod.ts:175-177; components/seo/ToolPage.astro:17-20
- Problema: Sitemap says 2026-10-29 while the MDX changed 2026-11-03.
- Reproducir: git log -1 -- docs/files/tools/ vs sitemap-0.xml.
- Arreglo propuesto: Add the docs MDX, tools.ts and tool-guides.ts to the fileLastmod list.

### S6 (T1) Changelog index and feeds silently stop at 50 releases
Card: https://app.notion.com/p/40e3a474adbb81628a44d9ee6584a562 · Confianza: Confirmado
- Evidencia: web/src/lib/changelog/query.ts:41 and callers
- Problema: After 50 releases older ones vanish from /changelog/ and feeds while the sitemap lists them.
- Reproducir: Code reading.
- Arreglo propuesto: Pagination or a link to older releases.

### S7 (T1) lastmod.json takes priority over git even in full checkouts
Card: https://app.notion.com/p/40e3a474adbb818b9c69d794cd38cefb · Confianza: Confirmado
- Evidencia: web/src/lib/sitemap/lastmod.ts:67
- Problema: Files changed after the last manual lastmod:sync keep the old date even when git is available (manifest already stale for its own entry).
- Reproducir: Comparison script: 1 stale entry.
- Arreglo propuesto: Prefer gitLastCommit when hasGit(); manifest only as fallback.

### S8 (T1) Entry titles are inserted unescaped into the feed HTML
Card: https://app.notion.com/p/40e3a474adbb81918a8ecff4b141afa7 · Confianza: Sospecha
- Evidencia: web/src/pages/changelog/rss.xml.ts:18; changelog.json.ts:22
- Problema: A title with \<, & or markup breaks or injects markup in feed readers (latent; no current title triggers it).
- Reproducir: grep titles for \[\<>&\`\] → none today.
- Arreglo propuesto: HTML-escape e.title in both feeds.

### S9 (T1) Entry slugs hash the title, so editing a title breaks permalinks and recreates the DB row
Card: https://app.notion.com/p/40e3a474adbb81938912e3579f20d4b3 · Confianza: Confirmado
- Evidencia: web/src/lib/changelog/parse.ts:61-64; import.ts:41-46
- Problema: A typo fix changes the slug (#anchors break, created\_at/updated\_at lost); duplicate titles get '-2' so the slug also depends on order.
- Reproducir: Code reading + tsx experiment.
- Arreglo propuesto: Allow an explicit \{#slug\} or a stable id.

### S10 (T1) Report-Only CSP lacks what Pagefind (wasm) and GA4 need
Card: https://app.notion.com/p/40e3a474adbb81dba0c4e59f9d75e472 · Confianza: Sospecha
- Evidencia: web/scripts/production-server.mjs:19-32
- Problema: 'wasm-unsafe-eval' missing in script-src; \*.[analytics.google.com](http://analytics.google.com) missing in connect-src; fills reports with noise and would break docs search if enforced.
- Reproducir: Pagefind/GA4 documented requirements; not run in a browser.
- Arreglo propuesto: Add 'wasm-unsafe-eval' and the analytics hosts.

### S11 (T1) Cache-Control rule targets /og.jpg which does not exist; /og/\*.png and og-welcome.jpg get no cache header
Card: https://app.notion.com/p/40e3a474adbb81f49dffc0b7fdd29304 · Confianza: Confirmado
- Evidencia: web/scripts/production-server.mjs:58-60
- Problema: Social cards revalidated on every fetch.
- Reproducir: ls dist/client: no og.jpg.
- Arreglo propuesto: Match \^/og-welcome.jpg\$ and \^/og/.

### S12 (T1) Broken anchor: ES Settings page links #updates but the ES heading is #actualizaciones; 'Check now' untranslated
Card: https://app.notion.com/p/40e3a474adbb81648c3def522943f060 · Confianza: Confirmado
- Evidencia: web/src/content/docs/es/docs/launcher/[settings.md:24](http://settings.md:24); es/docs/getting-started/[install.md:25](http://install.md:25)
- Problema: Only broken anchor across 492 pages.
- Reproducir: Link checker on dist.
- Arreglo propuesto: Link #actualizaciones; add a build-time anchor check.

### S13 (T1) www→apex redirect only on server-rendered routes and trusts the Host header
Card: https://app.notion.com/p/40e3a474adbb81bbb2d0cf23be5d0afb · Confianza: Confirmado
- Evidencia: web/src/middleware.ts:5-10
- Problema: Prerendered pages answer 200 on www.\*; on SSR routes any Host starting with www. redirects to the rest (open redirect if the proxy passes arbitrary Host).
- Reproducir: curl -H 'Host: [www.evil.com](http://www.evil.com)' /changelog/ → 301 [https://evil.com/changelog/](https://evil.com/changelog/).
- Arreglo propuesto: Redirect in production-server.mjs for every request and only for host === '[www.ferrocast.app](http://www.ferrocast.app)'.

### S14 (T1) web/scripts/migrate.mjs passes URL.pathname as a filesystem path
Card: https://app.notion.com/p/40e3a474adbb81c5b719fb3518cd678b · Confianza: Confirmado
- Evidencia: web/scripts/migrate.mjs:13
- Problema: A checkout path with spaces or non-ASCII becomes percent-encoded; migrations folder not found.
- Reproducir: Run from a path containing a space.
- Arreglo propuesto: fileURLToPath(new URL('../drizzle', import.meta.url)).

### S15 (T2) Clipboard.copy(\{html\}) copies an empty string; showToast(null) throws
Card: https://app.notion.com/p/40e3a474adbb8100b838e18103826c57 · Confianza: Confirmado
- Evidencia: extensions-runtime/src/api.js:146 (html-only → ''), :133 (typeof null === 'object' → new Toast(null))
- Problema: Clipboard silently cleared; \{file\} pasted as a path string; TypeError on null.
- Reproducir: Code reading.
- Arreglo propuesto: Fall back to stripped html text; guard a && typeof a === 'object'.

### S16 (T2) Release job runs third-party actions pinned to mutable refs, with contents:write and an unlocked signing keychain
Card: https://app.notion.com/p/40e3a474adbb8109883adb6ccd259a82 · Confianza: Confirmado
- Evidencia: .github/workflows/release.yml:8-9, 26 (dtolnay/rust-toolchain@master), 30, 101 (@v2), 52-56
- Problema: A compromised upstream action could push to master or codesign binaries once secrets exist.
- Reproducir: Config review.
- Arreglo propuesto: Pin to commit SHAs; scope contents:write to publish steps; delete the keychain right after [bundle.sh](http://bundle.sh).

### S17 (T2) Cache file write is not atomic, and a parse failure silently resets the cache
Card: https://app.notion.com/p/40e3a474adbb812eb28ccade82619fd9 · Confianza: Sospecha
- Evidencia: extensions-runtime/src/api.js:176 (catch → \{\}), :182 (writeFileSync in place); [sidecar.rs:111](http://sidecar.rs:111) (SIGKILL)
- Problema: A kill during the write leaves truncated JSON; next load treats it as empty and the next set overwrites it.
- Reproducir: Timing-dependent.
- Arreglo propuesto: Write to a temp file and rename.

### S18 (T2) CONTRIBUTING release steps are stale and conflict with [release.sh](http://release.sh) and the CI formula bump
Card: https://app.notion.com/p/40e3a474adbb8153ab7fe74d48b76514 · Confianza: Confirmado
- Evidencia: [CONTRIBUTING.md](http://CONTRIBUTING.md) Releasing steps 1-3; scripts/[release.sh](http://release.sh); .github/workflows/release.yml:116-130
- Problema: Following the doc skips the web/CHANGELOG sync and the annotated tag, and step 3 races the bot's formula push.
- Reproducir: Read the doc against the scripts.
- Arreglo propuesto: Replace with scripts/[release.sh](http://release.sh) X.Y.Z; state that CI updates the formula.

### S19 (T2) CI never runs the extensions-runtime tests nor checks that dist/runtime.js matches src
Card: https://app.notion.com/p/40e3a474adbb8191872dc8242ae5f00c · Confianza: Confirmado
- Evidencia: .github/workflows/ci.yml:16-40; crates/core/src/extensions/[mod.rs:14](http://mod.rs:14) embeds dist/runtime.js
- Problema: A src change committed without npm run build ships the old runtime silently; runtime regressions are not caught.
- Reproducir: Inspect ci.yml.
- Arreglo propuesto: Job: npm ci && npm run build && git diff --exit-code dist && npm test.

### S20 (T2) SHA256SUMS.txt omits the versionless Ferrocast.dmg asset
Card: https://app.notion.com/p/40e3a474adbb81db99e7e0751b7068dc · Confianza: Confirmado
- Evidencia: .github/workflows/release.yml:93-95
- Problema: No checksum line for the stable 'latest' URL file name.
- Reproducir: Read the step.
- Arreglo propuesto: Add Ferrocast.dmg to the shasum list.

### S21 (T2) Keyboard.Shortcut.Common.CopyPath differs from Raycast; CopyDeeplink and ToggleQuickLook missing
Card: https://app.notion.com/p/40e3a474adbb81f8829ad34db80c4ba3 · Confianza: Confirmado
- Evidencia: extensions-runtime/src/api.js:84 (CopyPath = ⌘⌥C); ToggleQuickLook undefined (observed)
- Problema: Shortcut mismatch and undefined constants.
- Reproducir: Experiment printed cmd+opt c and TQL undefined.
- Arreglo propuesto: Align with Raycast's table; add CopyDeeplink (⌘⇧C) and ToggleQuickLook (⌘Y).

### S22 (T2) [release.sh](http://release.sh) leaves a half-edited tree when cargo update or a git step fails
Card: https://app.notion.com/p/40e3a474adbb810c8abbf30cfe5bc09b · Confianza: Confirmado
- Evidencia: scripts/[release.sh:24](http://release.sh:24), 35-44 (edits first, cargo update --offline at :38 can fail under set -e); scripts/tests/release\_dry\_[run.sh](http://run.sh) has no Cargo.lock
- Problema: On failure Cargo.toml and CHANGELOG stay modified and a rerun refuses the dirty tree; if the push fails after tagging, rerun says 'tag exists'.
- Reproducir: Run with an empty offline registry cache.
- Arreglo propuesto: cargo update before editing or trap ERR to restore; add Cargo.lock to the dry-run fixture.

### S23 (T2) LocalStorage.getItem resolves null for missing keys (Raycast resolves undefined)
Card: https://app.notion.com/p/40e3a474adbb813690d6d47b77d51492 · Confianza: Confirmado
- Evidencia: crates/core/src/extensions/[storage.rs:63](http://storage.rs:63) (Value::Null), 128; extensions-runtime/src/api.js:157; bridge.js:51
- Problema: Extensions checking === undefined or default params treat a missing key as present null (JSON.parse(null) → null → property access crash).
- Reproducir: getItem('nope') → null.
- Arreglo propuesto: v ?? undefined in api.js getItem.

### S24 (T2) The capture helper is always built for arm64 while the app and other helpers use the host arch
Card: https://app.notion.com/p/40e3a474adbb81d281a5c553a38c4c31 · Confianza: Confirmado
- Evidencia: scripts/[bundle.sh:48](http://bundle.sh:48) (-target arm64-apple-macos13.0) vs :25, :65 (\$(uname -m)); Formula/ferrocast.rb has no arch restriction
- Problema: On an Intel Mac (brew builds from source) ferrocast-capture is arm64: capture and recording fail with 'Bad CPU type'.
- Reproducir: Code reading.
- Arreglo propuesto: -target "\$(uname -m)-apple-macos13.0" or depends\_on arch: :arm64.

### S25 (T3) Timezone parser accepts '13pm' and rejects '5 pm'
Card: https://app.notion.com/p/40e3a474adbb81088025f2398e23f8ae · Confianza: Confirmado
- Evidencia: crates/core/src/[timezones.rs:46](http://timezones.rs:46)-62, 88
- Problema: "13pm london in tokyo" → 10:00 PM; "5 pm london in tokyo" → no result.
- Reproducir: Scratch crate.
- Arreglo propuesto: Reject h>12 with am/pm; accept a space before am/pm.

### S26 (T3) Clipboard images copied from other Macs are written in place and never re-checked
Card: https://app.notion.com/p/40e3a474adbb8124b85dc9385ac412c0 · Confianza: Sospecha
- Evidencia: crates/core/src/sync\_[io.rs:676](http://io.rs:676), 684-689 (fs::copy straight to dst; exists() skips forever)
- Problema: An interrupted copy or a file iCloud is still writing leaves a truncated image never fetched again (note attachments use write\_atomic + content\_matches).
- Reproducir: Code reading.
- Arreglo propuesto: Copy to a temp file then rename.

### S27 (T3) Nested \{snippet\} placeholders are inserted unexpanded
Card: https://app.notion.com/p/40e3a474adbb816ca137fc4d3277401d · Confianza: Sospecha
- Evidencia: crates/core/src/[placeholders.rs:405](http://placeholders.rs:405); [app.rs:6848](http://app.rs:6848), 6864; [services.rs:841](http://services.rs:841)
- Problema: A snippet including another snippet with \{date\} pastes the literal "\{date\}".
- Reproducir: Code reading.
- Arreglo propuesto: Expand recursively with a depth limit.

### S28 (T3) '1 in to cm' returns nothing
Card: https://app.notion.com/p/40e3a474adbb8182a2a2f40db899e1fb · Confianza: Confirmado
- Evidencia: crates/core/src/[calc.rs:432](http://calc.rs:432)-434 (first separator ' in ' wins)
- Problema: evaluate("1 in to cm") = None.
- Reproducir: Scratch crate.
- Arreglo propuesto: Try each separator and keep the first whose right side is a known unit.

### S29 (T3) display\_title re-tokenises up to 200 KB per clipboard entry on every frame
Card: https://app.notion.com/p/40e3a474adbb819c9ffeeb420061f9b3 · Confianza: Confirmado
- Evidencia: crates/core/src/[models.rs:155](http://models.rs:155); [app.rs:4503](http://app.rs:4503)
- Problema: Up to 1000 entries × 200 KB split\_whitespace+join per frame.
- Reproducir: Code reading.
- Arreglo propuesto: Iterate lazily and stop after 120 chars.

### S30 (T3) frecency.json grows without limit
Card: https://app.notion.com/p/40e3a474adbb81b1825be470edae924f · Confianza: Confirmado
- Evidencia: crates/core/src/[ranking.rs:46](http://ranking.rs:46)-53 (every prefix of every query stored per item; no pruning)
- Problema: Long pasted queries add one key per character forever; the file is loaded/saved in full.
- Reproducir: Code reading.
- Arreglo propuesto: Cap prefix length (16) and key count; evict old ones.

### S31 (T3) Unused translation rows and a second Note::title that disagrees with display\_title
Card: https://app.notion.com/p/40e3a474adbb81c9bc26d9b493303e59 · Confianza: Confirmado
- Evidencia: strings.tsv rows "Replacing…" and "Convert ⇧" unused; [models.rs:68](http://models.rs:68)-71 Note::title returns "" for "###" where notes::display\_title returns "Untitled"
- Problema: Dead rows and a diverging helper.
- Reproducir: grep.
- Arreglo propuesto: Delete them or make Note::title call display\_title.

### S32 (T3) i18n::fill replaces a \{placeholder\} that appears inside an earlier value
Card: https://app.notion.com/p/40e3a474adbb8147b71ecabfdbe5f445 · Confianza: Confirmado
- Evidencia: crates/core/src/[i18n.rs:283](http://i18n.rs:283)-289 (sequential replace per argument)
- Problema: fill("Move \{name\} to \{folder\}", name="\{folder\}") → "Move Archive to Archive".
- Reproducir: Scratch crate.
- Arreglo propuesto: Single left-to-right pass.

### S33 (T3) HTML export keeps javascript: links
Card: https://app.notion.com/p/40e3a474adbb81d5bc43e99eb6246ec9 · Confianza: Confirmado
- Evidencia: crates/core/src/[markdown.rs:812](http://markdown.rs:812) (href = esc\_attr(url), scheme unchecked)
- Problema: \[x\](javascript:alert(1)) becomes a live javascript: link in the exported .html.
- Reproducir: markdown\_to\_html("\[x\](javascript:alert(1))").
- Arreglo propuesto: Allow only http, https, mailto, file and relative links.

### S34 (T3) Notes HTML export flattens nested lists
Card: https://app.notion.com/p/40e3a474adbb81e4aa27e3a7e86f40d2 · Confianza: Confirmado
- Evidencia: crates/core/src/[markdown.rs:991](http://markdown.rs:991)-1000
- Problema: "- a\\n  - nested\\n- b" exports as three sibling \<li>.
- Reproducir: markdown\_to\_html output.
- Arreglo propuesto: Track indentation and open nested \<ul>/\<ol>.

### S35 (T3) A second corrupt-file backup overwrites the first
Card: https://app.notion.com/p/40e3a474adbb81eb915cc76b0c7eac45 · Confianza: Confirmado
- Evidencia: crates/core/src/[storage.rs:43](http://storage.rs:43) rename(p, p.with\_extension("json.corrupt"))
- Problema: The earlier, possibly recoverable copy of user data is lost.
- Reproducir: Corrupt notes.json twice across launches.
- Arreglo propuesto: Timestamp/counter in the backup name, or skip when one exists.

### S36 (T3) Clipboard records '#123' or '#cafe' as a Color
Card: https://app.notion.com/p/40e3a474adbb81f89bc3c30b2caf76b9 · Confianza: Confirmado
- Evidencia: crates/core/src/[models.rs:139](http://models.rs:139)-141; [color.rs:88](http://color.rs:88)-100
- Problema: Issue references and hashtags show as colour swatches.
- Reproducir: ClipEntry::classify\_text("#123") = Color.
- Arreglo propuesto: Require 6 or 8 hex digits, or at least one a-f letter for short forms.

### S37 (T4) Unchecked CFString wrap of AX attributes can abort on a non-string value
Card: https://app.notion.com/p/40e3a474adbb811682c2f275d851e230 · Confianza: Sospecha
- Evidencia: crates/macos/src/[windows.rs:447](http://windows.rs:447), 500; [tiling.rs:301](http://tiling.rs:301)
- Problema: An app returning a non-CFString AXTitle/AXRole makes CFStringGetLength hit a non-string → abort.
- Reproducir: Needs a misbehaving AX app.
- Arreglo propuesto: CFType::wrap\_under\_create\_rule(v).downcast::\<CFString>().

### S38 (T4) Make Larger/Smaller panics on a screen narrower than 200 pt or shorter than 150 pt
Card: https://app.notion.com/p/40e3a474adbb814c96c0e93c841b032a · Confianza: Confirmado
- Evidencia: crates/macos/src/[windows.rs:220](http://windows.rs:220)-221 (clamp(200, s.w)); 479 unwrap
- Problema: f64::clamp panics when min > max.
- Reproducir: layout(MakeSmaller, w, Rect::new(0,0,150,100), 0.0) panics.
- Arreglo propuesto: clamp(200f64.min(s.w), s.w) and <code>?</code> instead of unwrap.

### S39 (T4) Export params file name collides between concurrent exports of one project
Card: https://app.notion.com/p/40e3a474adbb817bb871de7cdc6a411d · Confianza: Sospecha
- Evidencia: crates/macos/src/[capture.rs:335](http://capture.rs:335) (.cache/export-\{pid\}.ndjson), 344
- Problema: Two exports (MP4 + GIF) of one project overwrite and delete each other's parameters.
- Reproducir: Start two exports at once.
- Arreglo propuesto: Add the SEQ counter or request id to the name.

### S40 (T4) Shelf '−' button removes itself from inside its own action (possible use-after-free)
Card: https://app.notion.com/p/40e3a474adbb817f895fe4419e108abd · Confianza: Sospecha
- Evidencia: crates/macos/src/[shelf.rs:297](http://shelf.rs:297)-302 (refresh → removeFromSuperview inside the button's action)
- Problema: Classic AppKit pattern that can crash if the button is released during its own click handling.
- Reproducir: Unconfirmed; AppKit may retain the sender.
- Arreglo propuesto: Defer the refresh with performSelector afterDelay:0 / dispatch\_async.

### S41 (T4) Recording HUD sits over the Dock
Card: https://app.notion.com/p/40e3a474adbb81879f26d194165db944 · Confianza: Confirmado
- Evidencia: crates/macos/src/[hud.rs:174](http://hud.rs:174)-177 vs display\_frame 269-276 (always full frame())
- Problema: HUD placed 28 pt above the bottom of the full screen covers Dock icons.
- Reproducir: Start a recording with the Dock at the bottom.
- Arreglo propuesto: Use NSScreen.visibleFrame.

### S42 (T4) Notch player refresh rate comes from the primary screen, not the pill's screen
Card: https://app.notion.com/p/40e3a474adbb8199ac75edb784b8c867 · Confianza: Confirmado
- Evidencia: crates/macos/src/notch\_[player.rs:904](http://player.rs:904)-907 (screens()\[0\])
- Problema: With an external primary display the built-in ProMotion rate is ignored or the reverse.
- Reproducir: 60 Hz external primary + 120 Hz MacBook.
- Arreglo propuesto: Use panel.screen().

### S43 (T4) process::list says current user, but ps -ax lists every user; double spaces in paths collapse
Card: https://app.notion.com/p/40e3a474adbb81a1bb23ebf79a0db192 · Confianza: Confirmado
- Evidencia: crates/macos/src/[process.rs:44](http://process.rs:44) vs 46; parse\_ps 33
- Problema: Root daemons that cannot be killed appear; paths with double spaces parse wrong (wrong icon).
- Reproducir: Run Kill Process.
- Arreglo propuesto: Add -U \$UID (or filter by uid); parse comm from a fixed column.

### S44 (T5) 'Disabled commands' list shows internal ids instead of command names
Card: https://app.notion.com/p/40e3a474adbb8106adddc54ceb94a862 · Confianza: Confirmado
- Evidencia: crates/app/src/[draw.rs:8211](http://draw.rs:8211)-8219
- Problema: Raw ids like 'system:shut-down' or 'app:[com.apple](http://com.apple).Safari', untranslated.
- Reproducir: Disable a command → Settings → Advanced.
- Arreglo propuesto: Resolve the id to its item title (tr) before drawing.

### S45 (T5) Enter saves the Notion token but not the AI key or a theme rename
Card: https://app.notion.com/p/40e3a474adbb815c91c2f036b92e50ac · Confianza: Confirmado
- Evidencia: crates/app/src/[draw.rs:7887](http://draw.rs:7887)-7889 vs 7436-7441, 7452-7456, 10388-10394
- Problema: Inconsistent; keyboard users must tab to a small button.
- Reproducir: Type an API key and press ↵.
- Arreglo propuesto: Handle lost\_focus && Enter the same way for these fields.

### S46 (T5) /usr/bin/open is run with .status() on the UI thread
Card: https://app.notion.com/p/40e3a474adbb8189a06afcdd4c945728 · Confianza: Sospecha
- Evidencia: crates/app/src/[draw.rs:8452](http://draw.rs:8452), 1567
- Problema: The UI waits for LaunchServices; a hung Finder or editor freezes the launcher.
- Reproducir: Open data folder while Finder is busy.
- Arreglo propuesto: Use .spawn() as notes\_[ctl.rs:1415](http://ctl.rs:1415) does.

### S47 (T5) Delete All Notes runs with no confirmation
Card: https://app.notion.com/p/40e3a474adbb81c6a53ed5e52944d87b · Confianza: Confirmado
- Evidencia: crates/app/src/notes\_[ctl.rs:1002](http://ctl.rs:1002)-1005 → 784-791
- Problema: One palette pick moves every note to Recently Deleted; restoring is one by one.
- Reproducir: Notes → ⌘K → Delete All Notes.
- Arreglo propuesto: Add a confirm step in the palette.

### S48 (T5) Advanced tab tooltip says ⌘10; the shortcut is ⌘0
Card: https://app.notion.com/p/40e3a474adbb81f7bf2efc5c63349cca · Confianza: Confirmado
- Evidencia: crates/app/src/[draw.rs:7090](http://draw.rs:7090); 8658-8659
- Problema: Wrong hint for the 10th tab.
- Reproducir: Hover Advanced.
- Arreglo propuesto: Show (i + 1) % 10.

### S49 (T5) 'Reset this tab' Undo can only be clicked, and any other toast hides it
Card: https://app.notion.com/p/40e3a474adbb81568df0caa9ab4a561d · Confianza: Confirmado
- Evidencia: crates/app/src/[draw.rs:2115](http://draw.rs:2115)-2131, 2243, 84-86
- Problema: No keyboard undo; a toast in the next 8 s (sync, login item) removes the link while undo is still pending.
- Reproducir: Reset General with the login item on.
- Arreglo propuesto: Bind ⌘Z in Settings while settings\_undo is set; protect the Undo toast.

### S50 (T5) Translate 'Languages' field caches typed text and shows stale values after sync
Card: https://app.notion.com/p/40e3a474adbb8176b126ebbf8ee5e54c · Confianza: Sospecha
- Evidencia: crates/app/src/[draw.rs:7524](http://draw.rs:7524)-7533 (egui temp 'translate\_langs\_buf' cleared only on reset/undo: 10284, 10295)
- Problema: When translate\_languages changes elsewhere (iCloud merge, import) the field keeps the old text and the next keystroke writes it back over the synced list.
- Reproducir: Edit the field, then receive a sync change.
- Arreglo propuesto: Store (synced\_value, buf) as theme\_color\_row does and reset buf when the setting changes.

### S51 (T5) Hard-coded English text built with format! in the editor and media panels
Card: https://app.notion.com/p/40e3a474adbb81bda2c5fae11a753dad · Confianza: Confirmado
- Evidencia: crates/app/src/[draw.rs:2976](http://draw.rs:2976)-2979, 3291-3292, 3591-3600, 1537; editor\_[panels.rs:2657](http://panels.rs:2657), 415, 405, 1494
- Problema: Strings built with format!/painter.text/RichText bypass the i18n scanner and show in English.
- Reproducir: Language ja → Edit Metadata on an unreadable file; Visualize Audio → Still.
- Arreglo propuesto: Use trf/tr with named placeholders and add the rows.

### S52 (T5) AI key Save shows 'key stored' even when the Keychain write failed, and discards the typed key
Card: https://app.notion.com/p/40e3a474adbb81febebee1a04d1183d3 · Confianza: Confirmado
- Evidencia: crates/app/src/[draw.rs:8320](http://draw.rs:8320)-8335 (ai\_has\_key = now\_has regardless of ok; key\_scratch cleared); Notion does it right at [app.rs:2174](http://app.rs:2174)
- Problema: After a Keychain failure the row says a key is stored and the pasted key is gone.
- Reproducir: Locked keychain → Save.
- Arreglo propuesto: Use now\_has && ok; keep key\_scratch when the save failed.

### S53 (T5) Extension launch error toasts are built with format! and missing from strings.tsv
Card: https://app.notion.com/p/40e3a474adbb81c4a564d87fb4f04b12 · Confianza: Confirmado
- Evidencia: crates/app/src/ext\_[glue.rs](http://glue.rs) ext\_launch\_with / ext\_launch error strings; no rows in strings.tsv
- Problema: '… is turned off in Settings → Integrations', 'Required preferences are not set', 'Cannot start Node.js: \{e\}', etc. show in English.
- Reproducir: grep the tsv.
- Arreglo propuesto: Build with trf() and add rows.

### S54 (T6) Self-update can leave no app at the bundle path
Card: https://app.notion.com/p/40e3a474adbb81679018d0d644c63641 · Confianza: Sospecha
- Evidencia: crates/app/src/[services.rs:1561](http://services.rs:1561)-1576; [instance.rs:73](http://instance.rs:73)-81
- Problema: If the process dies after moving the bundle aside and before placing the new one (cross-volume ditto), /Applications/[Ferrocast.app](http://Ferrocast.app) is gone; .previous is deleted on next launch.
- Reproducir: Kill the app during a cross-volume copy.
- Arreglo propuesto: Copy the new app next to the bundle first, then two quick renames.

### S55 (T6) Timed-out commands can leave threads and child processes running
Card: https://app.notion.com/p/40e3a474adbb8175954ad8107974448b · Confianza: Sospecha
- Evidencia: crates/app/src/[services.rs:1412](http://services.rs:1412)-1440
- Problema: Only the direct child is killed; brew/ditto descendants keep running and drain threads block forever.
- Reproducir: brew upgrade hanging past 1200 s.
- Arreglo propuesto: Spawn in a new process group and kill the group; bound the drain threads.

### S56 (T6) Update error toasts are always English / Two user-facing messages are always in English (iCloud folder not found; JavaScript from Apple Events)
Card: https://app.notion.com/p/40e3a474adbb8175bd7fca819fa7390d · Confianza: Confirmado
- Evidencia: crates/core/src/[update.rs:91](http://update.rs:91), 96-97, 193 missing from strings.tsv; [app.rs:5876](http://app.rs:5876), 5915, 3412 \|\| crates/core/src/sync\_[io.rs:50](http://io.rs:50)-53 (via sync\_[ctl.rs:239](http://ctl.rs:239),271); [translate.rs:556](http://translate.rs:556) JS\_PERMISSION\_HINT ([app.rs:3928](http://app.rs:3928)); no rows in strings.tsv
- Problema: 'No connection to GitHub…', 'GitHub rate limit reached…', 'The app folder is not writable…', 'GitHub answered HTTP \{s\}' show in English; installer errors are formatted before tr. \|\| Shown in English in all 7 languages.
- Reproducir: Switch language, check for updates offline.
- Arreglo propuesto: Add rows, use trf for the HTTP code, tr() the installer error first. \|\| Route through tr/trf and add rows.

### S57 (T6) Repeated update checks stack duplicate Update screens
Card: https://app.notion.com/p/40e3a474adbb81b899effcf9ab0c5ac6 · Confianza: Sospecha
- Evidencia: crates/app/src/[app.rs:5904](http://app.rs:5904); 3389-3394
- Problema: Several View::Update pushed; Esc needed several times.
- Reproducir: Check for Updates twice with an update available.
- Arreglo propuesto: Replace the top view if it is already View::Update.

### S58 (T6) A half-written clipboard image is never repaired
Card: https://app.notion.com/p/40e3a474adbb81bdaae4c62c4c79d789 · Confianza: Sospecha
- Evidencia: crates/app/src/[services.rs:591](http://services.rs:591)-594 (write without temp file, error ignored, exists() check)
- Problema: A truncated PNG after a crash/disk-full is kept forever because later copies see the file exists.
- Reproducir: Fill the disk, copy a large image, free space, copy again.
- Arreglo propuesto: Write to .tmp and rename; rewrite when the size mismatches.

### S59 (T6) Updater verifies the zip in memory but extracts the copy on disk
Card: https://app.notion.com/p/40e3a474adbb81fc8c71ed31f08391c3 · Confianza: Sospecha
- Evidencia: crates/app/src/[services.rs:1536](http://services.rs:1536)-1543
- Problema: A same-user process can swap the zip between hash check and ditto.
- Reproducir: Theoretical race.
- Arreglo propuesto: Extract from the verified bytes in a 0700 dir, or re-hash right before ditto.

### S60 (T6) Running a script changes its file permissions to 0755
Card: https://app.notion.com/p/40e3a474adbb811ba908d31cc79bbba9 · Confianza: Confirmado
- Evidencia: crates/app/src/[services.rs:1294](http://services.rs:1294)-1307; [app.rs:4233](http://app.rs:4233)-4239 (inline auto-run)
- Problema: A script kept at 0600/0700 because it holds tokens becomes world-readable after one run or background refresh.
- Reproducir: chmod 700 a script, run it, ls -l.
- Arreglo propuesto: Only add the user execute bit when missing.

### S61 (T6) <code>return</code> in Msg::WebReplaced abandons the rest of drain()
Card: https://app.notion.com/p/40e3a474adbb813089a6e7f7fe91a763 · Confianza: Confirmado
- Evidencia: crates/app/src/[app.rs:3918](http://app.rs:3918)-3923 (<code>return</code> inside the loop at 3356; other arms use continue)
- Problema: After a successful web replace, queued messages stay in the channel with no repaint until another event.
- Reproducir: Web replace while other messages are queued.
- Arreglo propuesto: continue instead of return.

### S62 (T6) Script runs have no timeout and no output limit
Card: https://app.notion.com/p/40e3a474adbb81659bc7f1d9ef983cdd · Confianza: Confirmado
- Evidencia: crates/app/src/[services.rs:1314](http://services.rs:1314) (.output() no deadline); [app.rs:4227](http://app.rs:4227)
- Problema: A hung inline script never refreshes again until restart; chatty output is unbounded. run\_deadline exists but is unused here.
- Reproducir: Inline script <code>sleep 99999</code> with 10 s refresh.
- Arreglo propuesto: Use run\_deadline (60 s) and cap output.

## Tasks
- [x] T1 Website — S1-S14 — worker, branch fix/r4-web, worktree ../raycast-r4-web — commit: PR #126 merged
- [x] T2 CI, release scripts and extensions runtime — S15-S24 — worker, branch fix/r4-ci, worktree ../raycast-r4-ci — commit: PR #127 merged (verified; S17 already fixed by 099879b)
- [x] T3 Core — S25-S36 — worker, branch fix/r4-core, worktree ../raycast-r4-core — commit: PR #128 merged
- [x] T4 macOS — S37-S43 — worker, branch fix/r4-macos, worktree ../raycast-r4-macos — commit: PR #125 merged (S40, S42 not reproduced)
- [x] T5 App UI (Settings, notes, i18n) — S44-S53 — worker, branch fix/r4-ui, worktree ../raycast-r4-ui — commit: PR #129 merged
- [x] T6 App flows (updater, scripts, processes, clipboard) — S54-S62 — worker, branch fix/r4-flows, worktree ../raycast-r4-flows — commit: PR #130 merged (verified; 2 low findings fixed)

## Log
- L1 user: "sigamos"
- L2 parent: continues bugs-round-3 (closed fbb706f). Alta and Media are all Corregido; this round takes the 62 Baja cards. T2 (release/CI) and T6 (updater) touch delivery/security paths and get an independent verify before merge.
- L3 parent: closed. PRs #125 (T4), #126 (T1), #127 (T2), #128 (T3), #129 (T5), #130 (T6) merged; 62 Notion cards → Corregido (S17 already fixed by 099879b; S40 and S42 not reproduced, evidence in the T4 PR). T2 and T6 independently verified (T6's two low findings fixed before merge). CI caught a missing i18n row in T4 and a shallow-clone lastmod regression in T1; both fixed. Each branch re-merged with master and re-tested before merge. Board: Alta 7, Media 41, Baja 64 — all Corregido. Post-merge cargo test --workspace on master be1f3d6: 2764 passed, 0 failed.
- Follow-ups outside the cards: services.rs /usr/bin/open .status() calls (4) still on the UI thread; trim_row buttons and media_tools visualize_line untranslated; sync_ctl "Sync failed"/"Cannot use the sync folder" English-only; native review of new ja/zh rows; dry-run the split release workflow with an rc tag; CI does not run the DB-backed changelog tests.
