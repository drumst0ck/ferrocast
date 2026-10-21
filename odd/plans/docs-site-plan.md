<!-- Produced by claude-fable via el Gentleman (planning only). To be executed by Sonnet. Read odd/plans/web-relaunch-master-plan.md first. -->

# Documentation site at ferrocast.app/docs (Starlight, EN + ES)

Branch `chain/18-docs-site`. Goal: every capability the README status table and `CHANGELOG.md` describe has a documentation page that a user can find by search, and the long implementation notes that today bloat the README and the changelog move here.

## 1. Setup

- `npx astro add starlight` inside `web/`. Mount Starlight at `/docs` without changing the rest of the site: Starlight reads `src/content/docs/**`; pages under `src/pages/**` (home, files, changelog) stay as they are. Starlight generates routes from the content directory; prefix every slug with `docs/` by placing files under `src/content/docs/docs/…`? No: configure Starlight normally and keep the content at `src/content/docs/`, which produces routes at `/<slug>/`. To get `/docs/…`, put the files under `src/content/docs/docs/` (Starlight slug = path), or use the `site`/`base` of a nested integration. Decision: nest the files under `src/content/docs/docs/` and set the sidebar `autogenerate: { directory: 'docs' }`. UNVERIFIED on Starlight ≥ 0.30; confirm in the first commit with a one-page smoke test and adjust.
- i18n: Starlight `locales: { root: { label: 'English', lang: 'en' }, es: { label: 'Español', lang: 'es' } }`, Spanish files under `src/content/docs/es/docs/…`. Untranslated pages show Starlight's fallback banner automatically.
- Theme: override Starlight CSS variables to the site tokens (`web/src/styles/site.css:2-5`: `--bg #0f0f11`, `--accent #f0535f`, Stack Sans fonts self-hosted) in `src/styles/starlight.css`; custom `Header` component override that reuses `SiteHeader.astro` links (Home, Files, Docs, Changelog, GitHub) so the docs feel like the same site.
- Search: Pagefind is built in; nothing to add. Confirm it indexes both locales.
- `head` config: default OG image for docs (`/og-docs.jpg`, produced in the home plan), `twitter:card`.
- Starlight sitemap: Starlight adds `@astrojs/sitemap` itself; make sure only one sitemap integration is configured (merge with the SEO plan's config).

## 2. Information architecture (EN; ES mirrors it)

Sidebar groups and pages. Source of each page is cited so the writer copies facts, not impressions.

```
docs/
  index.mdx                       What Ferrocast is, who it is for, what it is not (README lines 1-4; FAQ "What is missing today?", index.astro:76)
  getting-started/
    install.md                    Homebrew from source, why a formula and not a cask, update, uninstall (README 6-24)
    first-run.md                  Welcome & Permissions, Accessibility, Input Monitoring, Automation for Finder (README "Qué está verificado", CHANGELOG "Convert Finder Selection")
    build-from-source.md          scripts/bundle.sh, dmg.sh, Rust 1.95, Apple Silicon only (README 26-38)
    migrating-from-raycast.md     Script Commands run as-is, snippet JSON import, quicklinks recreated, what is missing (index.astro FAQ rows 10-11)
  launcher/
    search.md                     Root search, frecency, aliases, favorites, hotkeys per command
    calculator.md                 Math, %, units, colours, URL detection, history (README table rows 2-3, CHANGELOG Fixed)
    currency-and-time-zones.md    ECB rates, 12 h cache, `time in tokyo`, `5pm ldn in sf`, fixed offsets est/pst/cet
    actions-panel.md              ⌘K, shortcuts
    clipboard.md                  Text/files/images, pin, password managers skipped, storage path and privacy
    snippets.md                   Placeholders {date}, {clipboard}, {cursor}, {snippet name=}, keyword rules
    quicklinks.md
    script-commands.md            Raycast-compatible metadata, modes full/compact/silent/inline, needsConfirmation, argument visibility warning (README "Qué está verificado" last bullet)
    window-management.md          ~40 commands, Accessibility
    system-commands.md            Wi-Fi, Keep Awake, Kill Process, timers, notes, emoji, Quick Look, Check for Updates
    ask-ai.md                     Four providers, Keychain, ChatGPT sign-in grey area wording verbatim from README row, Claude Code warm session timings (CHANGELOG)
    settings.md                   Six tabs, hotkeys, launch at login, backup export/import
    keyboard-shortcuts.md         Table from index.astro `keys` (lines 44-60) plus every ⇧⌘ editor shortcut from the README table
  files/
    index.md                      Overview: drag + Shift/Command/Control, local only, output next to the original never overwritten (file-tools.md S2, S7, S8)
    drag-wheel.md                 Three wheels, keys, Settings, halo design, keyboard route ⌥↵, Convert Finder Selection command
    conversions.md                Generated reference: 9 families, 46 inputs, 275 pairs, optional tools per pair (from conversions.json), limits paragraph from README row
    tools/                        One page per tool (26), generated skeleton + hand-written body:
      compress.md, edit-metadata.md, edit-photos.md, annotate.md, add-background.md, crop-image.md, redact-image.md, collage.md,
      trim-video.md, crop-video.md, video-speed.md, join-videos.md, video-snapshots.md, split-video.md, redact-video.md, mute-video.md,
      normalize-audio.md, audio-to-video.md, trim-audio.md, audio-channels.md, bleep-audio.md,
      merge-pdf.md, organize-pdf.md, split-pdf.md, create-pdf.md, extract-archive.md
    typed-commands.md             Every typed syntax in one place (`collage row 1600 gap 16`, `frame blur 40`, `edit brightness 20`, `redact 10,10 200x100 blur`, `500kb`, `40%`, `800x600`, `make Canon`, `gps 40.4,-3.7`, `trim 0:10-0:30`, `split 30s`, `mute`, `speed 1.5x`, `frame 1:23`, `normalize -14`, `bleep 0:05-0:07`, `pages 3,1,2r`) from CHANGELOG and README
    file-actions.md               Seven actions incl. Zip note (files.astro actionCopy, lines 24-32)
    shelf.md                      File Shelf list and floating panel, verification status
    optional-tools.md             ffmpeg, LibreOffice, cwebp, Ghostscript, unrar: what each unlocks, brew install lines, RAR x86 caveat
  reference/
    data-and-privacy.md           ~/Library/Application Support/Ferrocast, JSON unencrypted, FERROCAST_DATA_DIR, Keychain items, log file
    status.md                     The README status table moves here verbatim (one row per feature, "Implementado / no probado en sesión real" translated to the Built/Tested vocabulary); README keeps a short summary and links here
    environment-variables.md      FERROCAST_DATA_DIR, FERROCAST_QUERY, FERROCAST_PROFILE (README 32-34)
    troubleshooting.md            Accessibility re-prompt after upgrade, brew short form fails, Gatekeeper and DMG, ffmpeg missing hint, Reveal Log File
    faq.md                        Merge of index.astro faq (lines 62-75) and files.astro faq
  contributing/
    index.md                      From CONTRIBUTING.md, plus scripts/shot.sh, perf.sh, gallery.sh, test counts, chained PR convention
    architecture.md               crates/core, crates/macos, crates/app; egui; data flow for conversions (route.rs, convert.rs, services.rs per odd/plans/tangerine-parity-plan.md evidence)
```

About 55 pages EN. Spanish: translate `index`, `getting-started/*`, `files/index`, `files/drag-wheel`, `files/conversions`, `reference/faq`, `reference/troubleshooting` first (the pages Spanish searches land on); the rest falls back to EN with the banner and is translated in a follow-up commit.

## 3. Content rules

1. One page = one capability; first paragraph says what it does, second says how to invoke it (menu, shortcut, typed command, wheel), then options, then **Limits** (verbatim from the README row or CHANGELOG bullet), then **Status** badge (`Tested` / `Built` / `Typed`, same vocabulary as `web/src/data/status.ts:12-19`), then related links.
2. Long CHANGELOG bullets are the raw material: move the detail (for example the whole "Audio windows" bullet) into the tool pages and leave a one-sentence changelog entry (changelog plan §2.2).
3. Screenshots: reuse `web/public/img/files-*.jpg` (29 exist) through Starlight's `<Image>`; missing ones (Settings AI tab, Kill Process, Timers, Shelf panel, Choose Order, Extract, Split windows) are produced with `scripts/shot.sh` and listed in the home plan §6 asset list so they are taken once.
4. Every shortcut is written with macOS glyphs inside `<kbd>` (`⇧⌘A`), never spelled "Shift-Cmd-A".
5. No marketing voice in docs; comparisons with Raycast only on `migrating-from-raycast.md`.
6. Frontmatter: `title`, `description` (120–155 chars; add a content-collection schema check like `files.astro:22`), `sidebar.order`, optional `status: tested|built|typed` rendered by a small `StatusBadge` component (reuse `web/src/components/StatusBadge.astro`).

## 4. Generated reference pages

- `web/scripts/gen-docs.ts` (run in `prebuild`): reads `src/data/conversions.json` and `tools.json` and writes
  - `files/conversions.md` tables (family → inputs → outputs with a chip for the optional tool),
  - a `src/content/docs/docs/files/tools/_generated/<tool>.json` sidecar per tool (formats, shortcut, status) that each hand-written tool page imports through a `<ToolFacts id="compress" />` component, so the hand-written body never repeats data that can drift.
- The generation step must be deterministic and the output committed, so the Docker build does not need it; a CI check fails when running it produces a diff.

## 5. Navigation integration

- `SiteHeader.astro` gains a `Docs` link (`/docs/`) between Files and Changelog; `SiteFooter.astro` gains Docs, Changelog, RSS, Privacy.
- Home sections link to docs pages (`Learn more →`) instead of repeating detail (home plan §4).
- README "Documentation" section links to `/docs/` and three deep links (install, files, ask-ai).
- Starlight `editLink.baseUrl` → `https://github.com/drumst0ck/ferrocast/edit/master/web/src/content/docs/`.

## 6. Checks

- `npm run build` passes with both locales; Pagefind index exists in `dist/pagefind`.
- Link check: `npx linkinator dist --recurse` (or `astro-link-check`) reports 0 broken internal links; wire it into `npm test`.
- Description length schema enforced in `src/content.config.ts`.
- Lighthouse on `/docs/files/tools/compress/` ≥ 95 in all four categories.
- A human reads three pages against the README rows they come from and confirms no claim was upgraded (Built → Tested).

## Risk

Risk: none for the application code; the site change is additive. Content risk (overclaiming) is handled by rule 3.1 and the read-back check.
