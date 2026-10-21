<!-- Produced by claude-fable via el Gentleman (planning only). To be executed by Sonnet. Read odd/plans/web-relaunch-master-plan.md first. Search-volume figures are not included on purpose: they were not measured; validate the clusters in Google Search Console and a keyword tool before writing the second batch of pages. -->

# SEO plan for ferrocast.app

Branch `chain/20-seo` for the technical and programmatic parts; the content and distribution parts are an ongoing checklist the owner runs after launch.

Two audiences, two intent families:

1. **Launcher people**: searching for Raycast, Alfred, Spotlight alternatives, open-source launchers, "Raycast free alternative", "Raycast Rust", "launcher without account". Decision-stage queries; the comparison page and the home carry them.
2. **File people** (marketing, graphic design, video/social, documentation, students): searching for a task, not a product: "convert heic to jpg on mac", "compress video for email mac", "remove metadata from photo mac", "pdf to docx mac free", "blur face in video mac", "trim mp3 mac", "combine images into pdf mac". Task-stage queries with huge long-tail; programmatic pages carry them, and each one ends with the same answer: install Ferrocast, drag the file, tap Shift.

## 1. Positioning and keyword clusters

| Cluster | Target pages | Example queries (validate volumes) |
|---|---|---|
| Raycast alternative | `/`, `/compare/raycast/` | raycast alternative, raycast free alternative, raycast open source alternative, raycast vs alfred, launcher for mac open source, raycast without account |
| Spotlight/Alfred replacement | `/`, `/compare/alfred/` (later), docs `migrating-from-raycast` | alfred alternative free, spotlight replacement mac, mac launcher rust |
| Local AI launcher | `/docs/launcher/ask-ai/`, home block | ollama launcher mac, ask ai from spotlight, claude code launcher |
| File conversion (per pair) | `/convert/<a>-to-<b>/` ×275 (EN) + `/es/convertir/<a>-a-<b>/` | convert heic to jpg mac, webp to png mac, mov to mp4 mac free, pdf to docx mac, docx to pdf without word, epub to pdf mac, csv to xlsx mac |
| File tools (per tool) | `/tools/<tool>/` ×26 + ES | compress video mac free, reduce pdf size mac, remove exif from photo mac, crop video mac, blur face in photo mac, trim audio mac, split video into parts mac, normalize audio loudness mac, make collage mac, combine images into one pdf mac, reorder pdf pages mac |
| Format hubs | `/files/` and `/convert/images/` ×9 families | image converter mac, audio converter mac free, local file converter no upload |
| Privacy angle | `/docs/reference/data-and-privacy/`, FAQ | convert files without uploading, offline file converter mac |
| Tangerine-shaped queries | `/tools/`, `/files/` (no page named after the product) | drag file tap shift convert mac, file conversion wheel mac |

Rule: never target "raycast" as a brand term in titles; use it in H2s and body ("Coming from Raycast?") and on the comparison page only. Same for Tangerine: no page, no title; the comparison lives in the README log, not on the web.

## 2. Site architecture for search

```
/                        home (EN)                     1 hop to everything below
/files/                  hub: families, wheel, tools, actions
/convert/<family>/       9 family hubs (images, audio, video, pdf, documents, slides, spreadsheets, subtitles, archives)
/convert/<in>-to-<out>/  275 pair pages; breadcrumbs Home › Files › Images › HEIC to JPG
/tools/                  hub: 26 tools grouped by media
/tools/<tool>/           26 tool pages
/compare/raycast/        one comparison page (moves the table out of the home into a crawlable page; home keeps a summary)
/docs/…                  Starlight
/changelog/, /changelog/<version>/
/es/… mirrors with translated slugs: /es/convertir/heic-a-jpg/, /es/herramientas/comprimir-video/
```

Internal links: every pair page links to its family hub, its reverse pair (`jpg-to-heic` when it exists), the three most related pairs, and the tool pages for that family (compress, metadata, crop…). Every tool page links to the pairs of its formats. The home links to the hubs. This gives each page ≥ 5 inbound internal links.

## 3. Technical SEO (checklist for `chain/20-seo`)

1. `@astrojs/sitemap` with i18n config; `robots.txt` → `Sitemap: https://ferrocast.app/sitemap-index.xml`; disallow `/api/`.
2. `Base.astro`: `hreflang` alternates (`en`, `es`, `x-default`), `og:locale`, `twitter:title/description/image`, `og:site_name`, canonical always set (today only when passed, `Base.astro:16`), `<meta name="robots">` from `PUBLIC_ROBOTS` for staging.
3. JSON-LD by page type: `SoftwareApplication` (name, operatingSystem macOS, applicationCategory UtilitiesApplication, offers price 0, license MIT, softwareVersion from `Cargo.toml` `version`, downloadUrl = GitHub) on home; `FAQPage` on home and `/files/`; `HowTo` on every `/convert/*` and `/tools/*` (steps: install, drag, tap key, drop); `BreadcrumbList` on every non-home page; `TechArticle` on docs (Starlight default) ; `SoftwareApplication.releaseNotes` on changelog version pages. Validate with the Rich Results Test on three page types.
4. Per-page OG images: `astro-og-canvas` or `satori` at build for `/convert/*` and `/tools/*` ("HEIC → JPG on your Mac, locally"), the hand-made `og.jpg` for the rest.
5. Performance: self-host fonts, AVIF/WebP with `<picture>`, `loading="lazy"` below the fold, `fetchpriority="high"` on the hero image, no third-party scripts except analytics (§6), `Cache-Control: public, max-age=31536000, immutable` for `/_astro/*` set in the Node server (Astro's node adapter sets it for hashed assets; verify with `curl -I`).
6. `/llms.txt` and `/llms-full.txt` (plain-text index of docs and tool pages) for AI assistants; cheap and increasingly used for discovery.
7. 404 page keeps the header and search; log 404s in Coolify to catch broken inbound links after the migration.
8. Migration: GitHub Pages redirect shell (deploy plan §1.6); update README, Formula homepage, repo homepage; keep old URLs' equivalents at the same paths on the new domain (`/files/`, `/changelog/`).
9. Google Search Console: verify `ferrocast.app` (DNS TXT), submit the sitemap index, request indexing for home, `/files/`, `/compare/raycast/`, and 10 pair pages; Bing Webmaster Tools import from GSC.
10. Core Web Vitals monitored through GSC after 28 days; Lighthouse CI in `web-check.yml` with budgets (`lighthouserc.json`: performance ≥ 0.95 on `/`, `/files/`, one convert page).

## 4. Programmatic pages (the long-tail engine)

### 4.1 Data

`web/src/data/conversions.json` has `families[].inputs[].outputs[]` with the optional tool per pair (`files.astro` already renders chips). Add to the `site_data` example (`crates/core/examples/site_data.rs`) any field the pages need and that the matrix already knows: `needs` (ffmpeg|soffice|cwebp|unrar|ghostscript|null), `engine` (sips, afconvert, avconvert, textutil, ditto, pdfkit, ffmpeg, soffice…), `lossy: bool`, `notes` (e.g. "all pages at 300 DPI", "iWork '09 format", "RAR input only"). UNVERIFIED which of these the Rust matrix exposes today; `odd/plans/tangerine-parity-plan.md` cites `crates/core/src/matrix.rs` as the source.

### 4.2 Page template `/convert/[pair].astro` (`getStaticPaths` from the JSON)

- Title: `Convert HEIC to JPG on Mac, locally · Ferrocast` (≤ 60 chars); description generated from a sentence bank keyed by family with the pair's specifics (engine, dependency) so no two descriptions are identical.
- H1: "HEIC to JPG on your Mac, without uploading".
- Blocks: (1) the three-step HowTo with the wheel image; (2) "What happens under the hood": engine, lossy/lossless, dependency and its install line, the status badge; (3) "Limits" verbatim from the README conversion row where it applies (layout fidelity not checked, `.pages` iWork '09, RAR x86 caveat); (4) typed alternative (`Convert Files` command, `⌥↵`); (5) related pairs and tools; (6) 3 FAQs specific to the family (e.g. images: "Does it keep EXIF?", "What about transparency?"); (7) install block.
- Breadcrumb + `HowTo` + `BreadcrumbList` JSON-LD; OG generated.
- Index policy: index everything, but release in batches: families hubs + the 60 most likely pairs (images, video, pdf, documents) in the first deploy; the rest two weeks later after GSC shows the first batch as "Indexed". Pairs that are near-duplicates of each other (e.g. `wav-to-aiff`) still get a page but with the family FAQ varied by a seed so text differs.

### 4.3 Tool pages `/tools/[tool].astro` (from `tools.json`)

- Title pattern: `Compress video on Mac, free and offline · Ferrocast`; H1 with the task verb; blocks: what it does, how to open it (wheel ⌘, `⇧⌘…` shortcut, typed command), options, screenshot (`files-<id>.jpg`), limits verbatim, status, formats it accepts (from JSON), related pairs, FAQ ×3, install.
- Formats list links to the pair pages (compress → every image/audio/video/pdf input).

### 4.4 Comparison page `/compare/raycast/`

Move the comparison table from the home (`index.astro:16-25`) into its own page with 1,200+ words: who each is for, the honest "Raycast does far more" paragraph, the measured table and method, migration steps (link to docs), FAQ, disclaimer. The home keeps a 4-row summary and links here. Do not create `/compare/alfred/` until the Raycast page ranks (avoid spreading thin).

### 4.5 Spanish

Translated slugs through Astro i18n routing: `/es/convertir/<in>-a-<out>/`, `/es/herramientas/<slug>/`. Spanish sentence bank written once; family FAQs translated. Spanish has far less competition for "convertir heic a jpg mac" style queries; it is the cheapest win in this plan.

## 5. Content and distribution (owner checklist, post-launch)

Week 0 (launch):
- GitHub: repo description "Fast, open-source launcher for macOS with local file conversion (275 formats). Rust.", homepage `https://ferrocast.app`, topics `macos, launcher, raycast-alternative, rust, egui, file-converter, heic, ffmpeg, productivity, open-source`, social preview = new `og.jpg`, a Release `v0.1.0` with notes from the DB/CHANGELOG.
- Homebrew formula description and homepage updated (`Formula/ferrocast.rb`).
- Submit to: AlternativeTo (as alternative to Raycast, Alfred, Tangerine, CloudConvert), Product Hunt (launch day with the file-tools video), Hacker News "Show HN: Ferrocast – open-source Raycast-style launcher in Rust with local file conversion", r/macapps, r/rust, r/raycast (read rules; honest post), Lobsters, MacUpdate, Softpedia, awesome-mac and awesome-rust lists (PRs), Rust newsletter (This Week in Rust "call for participation"/"community").
- X/Mastodon/Bluesky threads with 20-second clips per tool (the asset list in the home plan).

Month 1–3 (content that earns links):
- Blog under `/blog/` (Astro content collection, EN/ES): "How Ferrocast converts HEIC without ffmpeg" (engineering), "Measuring a launcher: 2 ms hotkey to frame" (from `odd/plans/performance-baseline.md`), "Every macOS built-in CLI tool for file conversion" (sips, afconvert, avconvert, textutil: reference post that ranks on its own), "Redacting faces offline with Vision", "Raycast Script Commands in Ferrocast". Each post links to 3–5 tool/pair pages.
- Docs are link magnets too: the typed-commands reference and the optional-tools page get shared.
- Answer questions on Reddit/Stack Exchange ("convert heic to jpg mac terminal") linking the pair page only when it answers the question.

Ongoing:
- Release cadence drives changelog + RSS + a short post; each release gets a GitHub Release and a "What's new" entry.
- Monitor GSC monthly: pages with impressions but CTR < 2% get title rewrites; pages "Crawled, not indexed" after 60 days get merged into their family hub.

## 6. Measurement

- Analytics: self-hosted Umami or Plausible on Coolify (one more resource, cookie-less, no consent banner needed in the EU for either; Plausible needs ClickHouse, Umami only Postgres: choose **Umami** and reuse the Postgres server with its own database). Script tag in `Base.astro` behind `PUBLIC_ANALYTICS_URL`/`PUBLIC_ANALYTICS_ID`; events: `install_copy` (copy button), `wheel_demo_drop`, `rss_click`, `docs_search`.
- Search Console + Bing. Weekly export of queries to a sheet for the first 3 months.
- GitHub stars and Homebrew install counts (`brew info --analytics` is only for core; our tap has none: track `gh api repos/drumst0ck/ferrocast/traffic/clones` weekly).
- KPIs at 90 days: ≥ 100 indexed pages, ≥ 20 queries in top 20 for file-conversion intents, ≥ 3 queries in top 10 for "raycast alternative" family, referral traffic from ≥ 5 domains.

## 7. Checks for `chain/20-seo`

- `npm run build` emits `sitemap-index.xml` with EN+ES entries and `hreflang` alternates per URL.
- `/convert/heic-to-jpg/` passes Rich Results Test (HowTo + Breadcrumb); `/` passes (SoftwareApplication + FAQPage).
- No duplicate `<title>` or description across the 275+26+9 pages (build-time Set check).
- Lighthouse CI budgets pass.
- `curl -I` on a hashed asset shows the immutable cache header.
- Analytics event fires for `install_copy` in a Playwright run against the local build.

## Risk

Risk: item 3 (contract others consume: the `site_data` example output format feeds `files.astro` and `status.ts` count assertions; adding fields is additive, renaming is not; keep the 275/46 invariants). Verification: build must pass and `counts()` must not throw.
