<!-- Produced by claude-fable via el Gentleman (planning only, read-only on source). To be executed by Sonnet. Evidence is cited as path:line; anything marked UNVERIFIED was not checked. -->

# Ferrocast web relaunch: master plan

Umbrella plan for five deliverables. Each one has its own plan file; this document fixes the shared decisions, the target architecture, the order of execution and the acceptance criteria. Read this first, then the plan of the deliverable you are executing.

| # | Deliverable | Plan file | Depends on |
|---|---|---|---|
| A | Infrastructure: Coolify, Docker, PostgreSQL, domain, migration off GitHub Pages | `odd/plans/changelog-db-and-deploy-plan.md` (part 1) | nothing |
| B | Changelog database + API + `/changelog` page + RSS | `odd/plans/changelog-db-and-deploy-plan.md` (part 2) | A |
| C | Documentation site at `/docs` (Starlight, EN + ES) | `odd/plans/docs-site-plan.md` | A (domain), can start in parallel |
| D | Home page redesign with the new capabilities | `odd/plans/home-redesign-plan.md` | C (links to docs), B (changelog teaser) |
| E | SEO: technical, content, programmatic pages, distribution | `odd/plans/seo-plan.md` | A, C, D |
| F | README, CHANGELOG, CONTRIBUTING, in-app and GitHub metadata refresh | `odd/plans/readme-and-project-info-plan.md` | C (links), B (changelog format) |

## 1. Decisions (taken with the owner on 2026-10-21)

| Decision | Value | Consequence |
|---|---|---|
| Domain | `https://ferrocast.app` (already purchased) | All canonicals, sitemap, OG URLs, hreflang and the Homebrew/README links move to this origin. GitHub Pages (`https://drumst0ck.github.io/ferrocast/`) becomes a redirect shell. |
| Hosting | Coolify (owner's instance), Docker | One `web` application (Astro with the Node adapter, static pages prerendered, changelog on demand) plus one managed PostgreSQL resource. |
| Database | PostgreSQL on Coolify | Changelog is data, not Markdown at build time. `CHANGELOG.md` stays in the repo as the human-written source; a script pushes it into the DB. |
| Docs | Starlight inside the same Astro project, served at `/docs/` | One build, one domain, built-in search (Pagefind), sidebar and i18n. |
| Languages | English default (`/`), Spanish (`/es/`) with `hreflang` | Home, docs and programmatic SEO pages exist in both; technical artifacts stay English. README gets an English main file and a Spanish twin. |
| Base path | `/` (today `/ferrocast`, `web/astro.config.mjs:4`) | Every `${base}` usage keeps working because `base` collapses to an empty string; the `site` value changes. |

## 2. Current state (evidence)

- Website: Astro 5 + three.js, static, deployed to GitHub Pages by `.github/workflows/pages.yml` on push to `master` when `web/**` changes. Config: `web/astro.config.mjs` (`site: 'https://drumst0ck.github.io'`, `base: '/ferrocast'`).
- Pages: `web/src/pages/index.astro` (446 lines, hand-made home), `files.astro` (conversion explorer, 275 pairs), `changelog.astro` (imports `../../../CHANGELOG.md` at build time, 33 lines), `404.astro`.
- Shared layout `web/src/layouts/Base.astro` (38 lines): title, description, OG, canonical when passed, theme-color, one JSON-LD slot, Google Fonts (`Stack Sans Headline`/`Stack Sans Text`, `Base.astro:28-30`). No `hreflang`, no `twitter:title`, no sitemap integration (hand-written `web/public/sitemap.xml` with three URLs), hand-written `robots.txt`.
- Generated data: `web/src/data/conversions.json` and `tools.json` come from `cargo run -p ferrocast-core --example site_data` (`crates/core/examples/site_data.rs`); `web/src/data/status.ts` is the hand-written status vocabulary and throws at build time if the counts drift from 275/46. 26 tools (18 `window`, 8 `preset`), 7 actions.
- Design tokens: `web/src/styles/site.css:1-15` (dark ground `#0f0f11`, coral accent `#f0535f`, fluid type scale). Direction recorded in `.taste-skill/direction.json`.
- Screenshots already exist in `web/public/img/` for most file windows (`files-*.jpg`, 29 files) and launcher views; produced with `scripts/shot.sh` and `scripts/gallery.sh`.
- README (`README.md`, Spanish, 17.8 KB) is a status table with honest per-feature verification notes; `CHANGELOG.md` is one `0.1.0 — unreleased` block with a long flat bullet list; `docs/IMPLEMENTATION.md` and `docs/RESEARCH.md` are early-stage notes (29 and 63 lines).
- Repo: `github.com/drumst0ck/ferrocast`, default branch `master`, description "Fast keyboard launcher for macOS, written in Rust", homepage empty (checked with `gh repo view`). Work happens on `feat/ui-polish-and-fixes` with chained PRs `chain/NN-topic`.
- Installation is Homebrew-from-source (`Formula/ferrocast.rb`, README lines 6-14). No notarized download. The CTA of every page must stay the `brew tap` block.

## 3. Target architecture

```
ferrocast.app (Coolify, one Docker app "ferrocast-web")
├── /                 home (EN)            prerendered
├── /es/              home (ES)            prerendered
├── /files/           conversion explorer  prerendered  (+ /es/files/)
├── /convert/<a>-to-<b>/   275 programmatic pages (EN/ES)  prerendered from conversions.json
├── /tools/<tool>/    26 tool pages (EN/ES)               prerendered from tools.json
├── /docs/…           Starlight (EN) and /es/docs/…        prerendered, Pagefind search
├── /changelog/       rendered on demand from PostgreSQL   (+ /es/changelog/)
├── /changelog/rss.xml, /changelog.json, /api/changelog    on demand
├── /sitemap-index.xml, /robots.txt, /llms.txt             generated
└── healthz           for Coolify

Coolify PostgreSQL resource "ferrocast-db"  ←  scripts/changelog/push.ts (from CHANGELOG.md, run by CI on tag or by hand)
drumst0ck.github.io/ferrocast/*  →  static shell with <meta refresh> + canonical to ferrocast.app (GitHub Pages cannot 301)
```

Astro mode: `output: 'static'` with `@astrojs/node` adapter in `standalone` mode and `export const prerender = false` only on the changelog routes and API endpoints (Astro 5 on-demand rendering). This keeps the home and docs as plain HTML served by the Node server, and makes the changelog live without a second service.

## 4. Execution order and work units

Work on a branch per deliverable off `feat/ui-polish-and-fixes` (or `master` once that PR lands), chained PRs `chain/16-…` upward, about 400 authored lines each, tests and docs with the code, Conventional Commits.

| Order | Branch | Scope | Exit criterion |
|---|---|---|---|
| 1 | `chain/16-web-infra` | Dockerfile, Node adapter, `site: https://ferrocast.app`, `base: '/'`, healthcheck, Coolify app + DB created, DNS, GitHub Pages redirect shell, CI build check for `web/` | `https://ferrocast.app/` serves the current site over HTTPS; old URLs land on the new ones |
| 2 | `chain/17-changelog-db` | Drizzle schema + migrations, push script, API, `/changelog` on demand, RSS/JSON | `/changelog/` shows what `CHANGELOG.md` holds, from the DB, with a release date and categories |
| 3 | `chain/18-docs-site` | Starlight at `/docs`, EN content, generated reference pages, ES translation | Every feature in the README status table has a docs page; search works; `/es/docs/` mirrors EN |
| 4 | `chain/19-home-redesign` | New home sections, i18n routing for the home, new screenshots/video, FAQ, JSON-LD | Lighthouse ≥ 95 perf/SEO/a11y on mobile and desktop; all claims trace to README or CHANGELOG |
| 5 | `chain/20-seo` | Sitemap integration, hreflang, programmatic `/convert` and `/tools` pages, OG per page, analytics, Search Console, llms.txt | Search Console accepts the sitemap; 0 errors in the rich-results test on home, docs, a convert page |
| 6 | `chain/21-project-info` | README EN + ES, CHANGELOG restructure, CONTRIBUTING, docs/ folder cleanup, About screen links, repo metadata, social preview | README renders without stale links; `gh repo view` shows homepage and topics |

Steps 3 and 2 can run in parallel in separate worktrees (disjoint paths: `web/src/content/docs/**` vs `web/src/pages/changelog*`, `web/src/db/**`, `scripts/changelog/**`).

## 5. Shared rules for every deliverable

1. **Honesty is the brand.** Every claim on the web must trace to a row of the README status table or a `CHANGELOG.md` bullet. Keep the "Built / Tested / Typed" vocabulary of `web/src/data/status.ts`. Never say "verified in a real session" for something the README marks as not verified.
2. **Generated data stays generated.** Counts (275, 46, 26, 7) come from `conversions.json`/`tools.json` through `counts()` in `status.ts`, which throws when they drift. New pages that list tools or pairs must import those files, not copy numbers.
3. **No trademark misuse.** "Raycast", "Alfred", "Tangerine" are named for comparison only, in text; no logos, no "Raycast" in the brand name, no claim of affiliation. Comparison pages carry a one-line disclaimer.
4. **Install CTA is `brew tap … && brew install ferrocast`** (README lines 8-13). The DMG is not a public download; never offer it.
5. **Performance budget**: ≤ 120 KB JS per page excluding the 3D keys demo (which must stay lazy and behind `prefers-reduced-motion`), fonts self-hosted with `font-display: swap`, images as AVIF/WebP with explicit dimensions, LCP ≤ 2.0 s on a throttled mobile profile.
6. **i18n**: English is the default, served at `/`; Spanish under `/es/`. UI strings live in `web/src/i18n/{en,es}.ts`; content collections carry a `lang`. Every page emits `<link rel="alternate" hreflang>` for both plus `x-default`.
7. **Secrets** (`DATABASE_URL`, `CHANGELOG_ADMIN_TOKEN`, analytics IDs) only in Coolify environment variables and GitHub Actions secrets; never in the repo.
8. **Tests**: `npm run build` must pass in CI for `web/`; the changelog push script and the API get Vitest tests against a disposable Postgres (`docker compose -f web/docker-compose.test.yml`); content checks (description length, counts, link validity) run at build time the way `files.astro:22` already does.

## 6. Risks and how each plan handles them

| Risk | Where it is handled |
|---|---|
| GitHub Pages cannot send HTTP 301: SEO value of the old URLs decays | `changelog-db-and-deploy-plan.md` §1.6: meta refresh + canonical shell, Search Console change-of-address is not available for subpaths, so accept the loss and update every inbound link we control (README, Formula, repo homepage, CHANGELOG). |
| 275 programmatic pages read as thin content | `seo-plan.md` §4: each page composes real per-pair data (tools used, optional dependency, status, the typed command, limits from the README) plus family context; index family hubs first, release pairs in batches and watch Search Console coverage. |
| Claims drift between README, docs, home and DB | Single status vocabulary (`status.ts`); docs reference pages generated from the JSON; README status table moves to docs and README links to it. |
| Changelog DB becomes a second source of truth | The DB is populated only from `CHANGELOG.md` by the push script; hand edits in the DB are forbidden by convention and overwritten on the next push (upsert by `(version, slug)`). |
| Spanish copy drifts behind English | Docs translation status is tracked by Starlight's built-in fallback banner; home ES is a separate `index.astro` under `src/pages/es/` that imports the same data and a translated copy object; a build-time check fails when the EN and ES copy objects have different keys. |
| Coolify deploy breaks on a bad build | Dockerfile multi-stage with `npm ci && npm run build` in the image; Coolify healthcheck on `/healthz`; rollback is redeploying the previous image. |

## 7. Acceptance for the whole programme

- `https://ferrocast.app/`, `/es/`, `/files/`, `/docs/`, `/es/docs/`, `/changelog/`, one `/convert/…` and one `/tools/…` page return 200 over HTTPS with correct canonical and hreflang.
- Old GitHub Pages URLs redirect to their new equivalents.
- Google Search Console: property verified, sitemap index accepted, no "excluded by noindex" on pages meant to be indexed.
- Lighthouse mobile ≥ 95 on Performance, Accessibility, Best Practices, SEO for the home and a docs page.
- README (EN) links resolve; the Homebrew install block is identical in README, home and docs (a build-time check compares them).
- `CHANGELOG.md` and `/changelog/` show the same entries after running the push script.

## Log

- L1 (2026-10-21, owner, verbatim): "he elegido de modelo fable, vamos a hacer un planning para mejorar la home de la web con todo lo nuevo que hemos incluido en la herramienta, tambien tenemos que hacer un plan de SEO para posicinar la web en internet de la manera mas optima, necesitamos llegar a los usuarios de raycast y gente interesada en lo mismo, ademas nuestra herramienta de conversion de archivos hacer que podamos llegar a gente de marketing diseñadores graficos etc etc. Tambien debemos hacer la web de toda la documentacion de la herramienta, actualizar el readme y en general toda la info de la herramienta. Tambien levantaremos con el proyecto una db en la que empezaremos a subir todos los cambios de la herramienta para crear una pagina de changelog y mostrarlo ahi, toda la web, la documentacion y la db sera desplegada en coolify asi que podemos hacerlo con un docker o como creas conveniente. Elabora los planning de todo esto y luego lo ejecutare con sonnet"
- L2 (2026-10-21, owner answers): domain already purchased: `ferrocast.app`; database: PostgreSQL on Coolify; docs: Starlight inside the same Astro project at `/docs`; languages: English main + Spanish with hreflang.
