<!-- Produced by claude-fable via el Gentleman (planning only). To be executed by Sonnet. Read odd/plans/web-relaunch-master-plan.md first. Design work should load the `design-taste-frontend` skill and the direction already recorded in .taste-skill/direction.json. -->

# Home page redesign: ferrocast.app

Branch `chain/19-home-redesign`. The current home (`web/src/pages/index.astro`, 446 lines) sells a launcher; file tools are one bento card ("File tools", `index.astro:42`) and one section (`#files`, `index.astro:180-189`). Since it was written the app gained: 275 conversions, 26 tool windows (18 `window`, 8 `preset` in `tools.json`), 7 file actions, the three-wheel drag model with the halo design, the floating shelf, the PDF organizer, the audio windows, Ask AI with four providers and a model switcher, Kill Process, timers, performance work. The home must now speak to two audiences at once: keyboard launcher users (Raycast/Alfred/Spotlight people) and people who convert, compress, crop and redact files all day (marketing, design, video, social media, documentation teams).

## 1. Audit of the current home

| Section (`index.astro`) | Keep / change |
|---|---|
| Hero "Launch, calculate, paste and ask." (`:97-109`) + 3D keys canvas | Change headline to carry both promises; keep the keys demo but make it lazy and reduced-motion aware (budget rule). |
| Live launcher demo `.stage` (`:111-126`) | Keep; add a second demo state that shows a file being converted (typed `heic to jpg` style) cycling with the currency one. |
| Credo (`:128-131`) | Keep, rewrite to three lines: fast, local, honest. |
| "Why not just use Raycast?" compare table (`:133-143`, data `:16-25`) | Keep: it is the strongest SEO asset for the Raycast audience. Add rows "File conversion" and "File tools" (Raycast: none built in / via extensions). Add disclaimer line. |
| Stats `.stats` (`:145-152`) | Replace the four with data-driven counters: 275 conversions · 26 tools · ~40 window commands · 8.5 MB; counters come from `counts()` in `status.ts`. |
| Bench "Measured, not promised" (`:154-169`) | Keep as is; move lower (after features). Update date and numbers only if re-measured. |
| Features bento `#features` (`:171-178`) | Rewrite cards (see §4). |
| `#files` section (`:180-189`) | Promote to a full "Files" block with the wheel demo (`WheelDemo.astro` already exists on `/files`) and the three keys. |
| Story/tour `#tour` (`:191-208`, data `views` `:26-33`) | Keep, add two steps: "Drop a file" (wheel) and "Edit" (one of the editors). |
| Video `.vid` (`:210-222`) | Keep; record a second clip for file tools (asset list §6). |
| "Small things" list (`:224-229`, data `extras` `:34-44`) | Keep, trim to 8, move "File tools" out (it has its own block). |
| Shortcuts `#shortcuts` (`:231-239`) | Keep; link to docs keyboard page. |
| Install `#install` (`:241-258`) | Keep verbatim command block (shared constant). |
| FAQ (`:260-266`, data `:62-75`) | Keep; add 3 questions aimed at the file audience (§4.11); emit `FAQPage` JSON-LD. |
| CTA (`:268-282`) | Keep. |
| Header/footer | Use `SiteHeader`/`SiteFooter` with Docs link and language switch. |

## 2. Narrative and headline

One sentence that holds both audiences, measured not hyped:

- H1 (EN): **"The fast launcher that also converts your files."** Sub: "Search, calculate, paste and ask AI from the keyboard. Drag any file, tap a key, and convert, compress, crop, trim or redact it on your Mac. Open source, 8.5 MB, nothing leaves your disk."
- H1 (ES): **"El lanzador rápido que además convierte tus archivos."** Sub in neutral Spanish.

Alternatives to A/B later (keep in a comment): "Raycast speed. Tangerine's file tools. Zero accounts." (uses third-party names: no), "Launch, calculate, paste, convert." (closer to the current one).

Page order (top → bottom): Hero → Live demo → Files block (new, big) → Why not Raycast → Features bento → Tour → Video → Measured → Ask AI block (new, small) → Small things → Shortcuts → Install → FAQ → Changelog teaser (new) → CTA.

## 3. Layout and design direction

Keep the recorded direction (`.taste-skill/direction.json`, "product-performs", coral on dark) and the tokens in `site.css`. Load `design-taste-frontend` before touching CSS; its inspector runs on the built page. Constraints:

- No new fonts; self-host Stack Sans (download the two weights into `web/public/fonts/`, `font-display: swap`, drop the Google Fonts `<link>` in `Base.astro:28-30`). This alone removes a render-blocking third-party request.
- Section rhythm stays `--gap-sec`; the Files block may break the grid with a full-bleed dark stage that hosts the wheel.
- Light mode is already supported through `color-scheme: dark light` (`Base.astro:20`); the new sections must be checked in both.
- Motion: reveal-on-scroll (`reveal.js`) and the keys canvas only when `prefers-reduced-motion: no-preference`.

## 4. Section specs

### 4.1 Hero
- H1/sub above; two CTAs: primary "Install with Homebrew" (scrolls to `#install`), secondary "See the file tools" (`#files`). Proof line under the CTAs: "MIT · Apple Silicon · no account · {n.pairs} conversions" from `counts()`.
- Right/below: the existing keys canvas, lazy-loaded (`client:visible` equivalent: dynamic `import()` on intersection), hidden on `prefers-reduced-motion`.

### 4.2 Live demo (existing `.stage`)
- Rotate three queries every 6 s: `100 usd in eur`, `time in tokyo`, and a file row that shows the Convert view with `heic → jpg` chips. Pause on hover/focus. The file state is a static mock rendered from `conversions.json` (image family outputs).

### 4.3 Files block (new, replaces `#files`)
- Heading: "Drag a file. Tap a key. Done." Three key cards (⇧ Convert · ⌘ Tools · ⌃ Actions) reuse `KeysExplainer.astro`.
- Center: `WheelDemo.astro` with the halo wheel, interactive (already built for `/files`).
- Under it, three columns with real screenshots: Convert (explorer teaser with family chips and "275 pairs across 46 formats"), Tools (grid of 8 tool thumbnails: compress, edit photos, crop, redact, trim video, organize PDF, trim audio, collage; each links to `/tools/<id>/`), Actions (7 chips).
- Honesty line from `statusLine` (`status.ts:30`) in small print.
- CTA: "Explore every conversion →" (`/files/`) and "Read the docs →" (`/docs/files/`).

### 4.4 Why not Raycast (existing)
- Add rows: `File conversion` ("275 pairs, local, built in." vs "Not built in; some extensions upload files."), `File tools` ("26 tools: compress, crop, trim, redact, organize PDF…" vs "Not built in."). UNVERIFIED claim about extensions uploading: write "depends on the extension" instead unless verified.
- Add a one-line disclaimer under the table: "Raycast is a trademark of Raycast Technologies Ltd. Numbers measured on {bench.date}; see method below." Keep the respectful tone of the current intro (`index.astro:157`).

### 4.5 Features bento (existing `#features`)
Eight cards, each with a verb headline, two lines and a docs link:
1. Search everything (apps, commands, files, frecency, aliases)
2. Calculate in the search box (math, units, currencies, time zones, colours, history)
3. Clipboard history (pin, images, password managers ignored)
4. Snippets and quicklinks (placeholders, auto-expansion, arguments)
5. Ask AI, your way (Ollama by default, OpenAI-compatible, Anthropic API, Claude Code, ChatGPT sign-in; keys in Keychain)
6. Window management (~40 layouts)
7. Script Commands, Raycast-compatible
8. System: Kill Process, timers, Wi-Fi, Keep Awake, Quick Look

### 4.6 Tour (existing `#tour`)
Add steps `['Drop a file', 'files-wheel-convert', '…']` and `['Edit', 'files-edit', 'Nine sliders with a live preview; ↵ writes a full-size copy, never over the original.']`. Images exist in `web/public/img/`.

### 4.7 Video
Second clip (asset §6): drag a HEIC onto the launcher, tap ⇧, drop on JPG, toast with Reveal; then ⇧⌘K crop editor. 20–30 s, muted, poster. Caption keeps the "No mock-ups. This is the app." line.

### 4.8 Measured (existing)
Unchanged content; add a row "Conversion HEIC → JPG, 12 MP" only if measured with `scripts/perf.sh` or a one-off timing (record method); otherwise do not add.

### 4.9 Ask AI block (new, compact)
Two columns: provider chips (Ollama, LM Studio, OpenAI-compatible, Anthropic API, Claude Code, ChatGPT sign-in) and the privacy sentence from the FAQ ("Out of the box it talks to Ollama on your own Mac"). Link to `/docs/launcher/ask-ai/`. Keep the "grey area" caveat for ChatGPT sign-in in one honest sentence.

### 4.10 Changelog teaser (new)
Latest release version, date and the first three "Added" titles, fetched at build time from `/api/changelog` when `DATABASE_URL` is set, else from `CHANGELOG.md` headers (same fallback as the sitemap). Link "All changes →" and the RSS icon.

### 4.11 FAQ additions (file audience)
- "Which formats can it convert?" → families line + link.
- "Do I need ffmpeg?" → macOS tools first; ffmpeg/LibreOffice/Ghostscript optional, hint shown in app.
- "Does it overwrite my originals?" → never; copies next to the original with " 2", " 3".
- "Can I compress a video to a target size?" → yes, computed bitrate, rough estimate (verbatim limit from CHANGELOG).
Emit `FAQPage` JSON-LD from the array (SEO plan §3).

### 4.12 Install
Single shared constant `web/src/data/install.ts` exported to home, docs install page and README check script (master plan §7).

## 5. i18n of the home

- `web/src/pages/index.astro` becomes a thin page that imports `HomePage.astro` with a `t` copy object; `web/src/pages/es/index.astro` does the same with the Spanish copy. Copy objects live in `web/src/i18n/home.en.ts` and `home.es.ts`; a Vitest test asserts both have identical key sets.
- Language switch in `SiteHeader` (EN/ES) pointing to the equivalent URL; `Base.astro` emits `hreflang` for `en`, `es`, `x-default` from a `alternates` prop.
- Spanish copy is neutral/professional; file-tool vocabulary: convertir, comprimir, recortar (crop), cortar (trim), dividir, unir, redactar/ocultar, anotar, organizar páginas.

## 6. Assets to produce (once, shared with docs and SEO)

| Asset | How |
|---|---|
| `og.jpg` (1200×630) new, with the wheel and the headline; `og-docs.jpg`; `og-convert.jpg` template for programmatic pages | Design in the editor of choice or render with `satori` at build (`astro-og-canvas`); the SEO plan prefers generated per-page OG for `/convert/*`. |
| Video clip 2 (files) | Screen recording on the maintainer's Mac; `ffmpeg` to 1080p H.264 + AV1/WebM, poster frame. |
| Missing screenshots: Kill Process, Timers, Settings → AI, Choose Order, Split, Extract result toast, Shelf panel, Convert Finder Selection | `scripts/shot.sh out.png FERROCAST_QUERY=…` and `scripts/gallery.sh`; export JPG q80 and AVIF; 2× for Retina with `width/height` attributes. |
| Tool thumbnails (8) | Crops of existing `files-*.jpg`. |

## 7. Checks

- Build-time: description length 140–155 on both locales; copy-key parity test; `counts()` assertions; install block parity with README.
- Lighthouse mobile/desktop ≥ 95 ×4 on `/` and `/es/`; CLS < 0.05 (explicit image sizes; fonts swapped).
- `design-taste-frontend` inspector pass with no slop findings (generic gradients, centered-everything, emoji icons).
- Axe: 0 serious issues; the wheel demo is keyboard-operable or marked decorative with a text equivalent.
- Visual check in dark and light, 390 px and 1440 px, with Playwright screenshots stored under `web/tests/__screenshots__/` (not committed binaries if >1 MB; otherwise committed).
- Honesty read-back: every sentence in the Files block and FAQ traced to a README row or CHANGELOG bullet (list them in the PR description).

## Risk

Risk: none (presentation layer; shared `Base.astro` changes are covered by the build and the link check).
