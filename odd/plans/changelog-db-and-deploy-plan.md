<!-- Produced by claude-fable via el Gentleman (planning only). To be executed by Sonnet. Read odd/plans/web-relaunch-master-plan.md first. -->

# Infrastructure (Coolify + Docker + domain) and changelog database

Two parts. Part 1 is the hosting move (`chain/16-web-infra`); part 2 is the changelog data layer (`chain/17-changelog-db`). Part 2 needs part 1 only for the PostgreSQL resource; its code can be written against a local `docker compose` Postgres.

## Part 1: hosting on Coolify at ferrocast.app

### 1.1 Astro configuration

`web/astro.config.mjs` today:

```js
site: 'https://drumst0ck.github.io', base: '/ferrocast', devToolbar: { enabled: false }
```

Target:

```js
import { defineConfig } from 'astro/config';
import node from '@astrojs/node';
import sitemap from '@astrojs/sitemap';
import starlight from '@astrojs/starlight';   // added by the docs plan; leave the import point ready

export default defineConfig({
  site: 'https://ferrocast.app',
  base: '/',
  output: 'static',                 // Astro 5: static by default, on-demand per route with prerender=false
  adapter: node({ mode: 'standalone' }),
  trailingSlash: 'always',          // the current sitemap and links use trailing slashes (web/public/sitemap.xml)
  i18n: { defaultLocale: 'en', locales: ['en', 'es'], routing: { prefixDefaultLocale: false } },
  integrations: [sitemap({ i18n: { defaultLocale: 'en', locales: { en: 'en', es: 'es' } } })],
  devToolbar: { enabled: false },
});
```

- Every file already derives `base` with `import.meta.env.BASE_URL.replace(/\/$/, '')` (`index.astro:7`, `files.astro:18`, `changelog.astro:4`, `Base.astro:5`, `SiteHeader.astro:4`); with `base: '/'` that expression yields `''` and links keep working. Grep for the literal `drumst0ck.github.io` (`Base.astro:22`, `files.astro:19`, `public/robots.txt`, `public/sitemap.xml`) and replace with `Astro.site`.
- Delete `web/public/sitemap.xml` (the integration generates `sitemap-index.xml`); rewrite `robots.txt` to point at `https://ferrocast.app/sitemap-index.xml`.
- Add `web/src/pages/healthz.ts` returning `200 ok` with `prerender = false`.

### 1.2 Dockerfile (`web/Dockerfile`)

Multi-stage, Node 22 (CI already uses 22: `.github/workflows/pages.yml:29`).

```
FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build          # runs the data-count assertions in status.ts; a drift fails the image

FROM node:22-alpine AS run
WORKDIR /app
ENV NODE_ENV=production HOST=0.0.0.0 PORT=4321
COPY --from=build /app/dist ./dist
COPY --from=build /app/node_modules ./node_modules   # prune to production deps: npm ci --omit=dev in a separate stage
COPY package.json .
USER node
EXPOSE 4321
HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO- http://127.0.0.1:4321/healthz || exit 1
CMD ["node", "./dist/server/entry.mjs"]
```

Add `web/.dockerignore` (`node_modules`, `dist`, `.astro`). The generated JSON data is committed, so the image does not need Rust.

UNVERIFIED: whether `three` is needed at runtime; it is a client bundle dependency, so a production prune keeps it in `dependencies` and that is fine.

### 1.3 Coolify resources

1. **PostgreSQL** resource `ferrocast-db` (Postgres 16), internal network only, daily backups enabled in Coolify, note the internal `DATABASE_URL`.
2. **Application** `ferrocast-web`: source = GitHub repo `drumst0ck/ferrocast`, branch `master`, build pack = Dockerfile, base directory `/web`, port `4321`, healthcheck path `/healthz`. Environment variables: `DATABASE_URL`, `CHANGELOG_ADMIN_TOKEN` (random 32 bytes), `PUBLIC_SITE_URL=https://ferrocast.app`, `PUBLIC_ANALYTICS_DOMAIN` (see SEO plan).
3. Domains: `https://ferrocast.app` and `https://www.ferrocast.app` with Coolify's Let's Encrypt; configure a redirect `www → apex` (Coolify proxy label or a tiny middleware in `web/src/middleware.ts` that 301s when `host` starts with `www.`; the middleware is the portable choice).
4. Auto-deploy: enable the GitHub webhook in Coolify so a push to `master` deploys. Keep the deploy filtered to `web/**` changes if Coolify supports path filters; otherwise accept rebuilding on every push (about 1 minute).
5. Optional: a `staging` application on branch `feat/ui-polish-and-fixes` with `robots: noindex` through `PUBLIC_ROBOTS=noindex` read by `Base.astro`.

DNS at the registrar: `A`/`AAAA` for apex and `CNAME www` to the Coolify host (values UNVERIFIED, owner supplies them).

### 1.4 CI changes

- Replace `.github/workflows/pages.yml` with `web-check.yml`: on PRs and pushes touching `web/**`, run `npm ci && npm run build && npm test` on `ubuntu-latest`, plus `docker build web` to make sure the image builds. Deployment is Coolify's job.
- Keep `ci.yml` (Rust) unchanged.

### 1.5 Local development

`web/docker-compose.yml` with `postgres:16` on `5432`, volume `ferrocast-pg`, and `web/.env.example` documenting `DATABASE_URL=postgres://ferrocast:ferrocast@localhost:5432/ferrocast`, `CHANGELOG_ADMIN_TOKEN=dev`. `npm run dev` works without the DB for every page except `/changelog` (which shows an empty-state message when the DB is unreachable, never a 500).

### 1.6 Leaving GitHub Pages

GitHub Pages cannot issue HTTP 301. Replace the Pages deployment with a tiny static site (`web-redirect/`, or a `gh-pages` branch) that holds, for each old URL (`/ferrocast/`, `/ferrocast/files/`, `/ferrocast/changelog/`, `/ferrocast/404.html`), an HTML file with:

```html
<meta http-equiv="refresh" content="0; url=https://ferrocast.app/files/">
<link rel="canonical" href="https://ferrocast.app/files/">
<a href="https://ferrocast.app/files/">ferrocast.app/files</a>
```

Google treats an instant meta refresh plus canonical as a permanent redirect for ranking purposes. Keep the shell for at least 12 months. Update every inbound link we control: README, `Formula/ferrocast.rb` `homepage`, repo homepage (`gh repo edit --homepage https://ferrocast.app`), `CHANGELOG.md`, the About screen in the app (search `crates/app/src` for `drumst0ck.github.io`; UNVERIFIED whether the app links to the site).

### 1.7 Tests and checks for part 1

- `docker build web` succeeds in CI.
- `curl -I https://ferrocast.app/healthz` → 200; `curl -I https://www.ferrocast.app/` → 301 to apex.
- `curl -s https://ferrocast.app/sitemap-index.xml` lists `sitemap-0.xml` containing `/`, `/files/`, `/changelog/`.
- Old `https://drumst0ck.github.io/ferrocast/files/` lands on `https://ferrocast.app/files/`.

## Part 2: changelog in PostgreSQL

### 2.1 Source of truth and flow

`CHANGELOG.md` remains the file humans edit (it is also what GitHub shows). The database is a projection of it, populated by `scripts/changelog/push.ts` and read by the website. Two triggers:

1. **Manual**: `npm run changelog:push -- --file ../CHANGELOG.md` with `DATABASE_URL` and `CHANGELOG_ADMIN_TOKEN` (posts to `/api/changelog/import`, so no DB access from the laptop is needed).
2. **CI**: GitHub Action `changelog-push.yml` on push to `master` touching `CHANGELOG.md`, and on tag `v*`, posts the parsed file to the production import endpoint with the secret token.

Why not write Markdown straight from Astro at build time? Because the owner asked for a database that grows independently of deploys (L1), and because the same table can later feed an in-app "What's new" view (the app already has an update check through Homebrew, README line 37) and the RSS feed without rebuilding the site.

### 2.2 Restructure `CHANGELOG.md` so it parses

Today the file is one `## 0.1.0 — unreleased` block with `### Added in the latest iteration`, `### Fixed`, `### Initial feature set` and very long bullets. Move to Keep a Changelog form so a parser is trivial and GitHub readers still get the same content:

```md
## [Unreleased]

## [0.1.0] - 2026-10-21
### Added
- **File conversion**: 275 pairs across 46 input formats … (one feature per bullet, ≤ 3 sentences; details go to docs)
### Changed
### Fixed
### Known limitations
```

Rules: `## [version] - YYYY-MM-DD` headers, `### Added|Changed|Fixed|Removed|Security|Known limitations` groups, one bullet per entry, an optional `**Title**:` prefix becomes the entry title, an optional trailing `(docs: /docs/files/compress/)` link becomes `docs_url`, optional `#tags` at the end of the bullet (`#files #video`) become tags. The long current bullets are split: the first sentence stays in the changelog, the rest moves to the relevant docs page (docs plan §3).

### 2.3 Schema (Drizzle ORM, `web/src/db/schema.ts`)

```
releases   id serial PK · version text UNIQUE · released_on date NULL (NULL = unreleased) · title text NULL · summary text NULL · created_at · updated_at
entries    id serial PK · release_id FK → releases ON DELETE CASCADE · category enum(added, changed, fixed, removed, security, limitation) · slug text (stable hash of version+title, UNIQUE with release_id) · title text NULL · body_md text · body_html text · docs_url text NULL · position int · created_at · updated_at
tags       id serial PK · name text UNIQUE
entry_tags entry_id FK · tag_id FK · PK(entry_id, tag_id)
imports    id serial PK · source_sha text · imported_at · entries_count int   (audit of each push)
```

Migrations with `drizzle-kit generate` committed under `web/drizzle/`; applied at container start by `node dist/server/entry.mjs` wrapper? No: keep start simple and run `npm run db:migrate` as a Coolify *pre-deploy command* (Coolify supports it) or as the first step of the import endpoint guarded by a lock. Decision: pre-deploy command; document it in the Coolify app notes.

`body_html` is rendered at import time with `marked` + `sanitize-html` so the page does no Markdown work per request.

### 2.4 API (`web/src/pages/api/changelog/*.ts`, all `prerender = false`)

| Route | Method | Auth | Purpose |
|---|---|---|---|
| `/api/changelog/import` | POST JSON `{ sourceSha, releases: [...] }` | `Authorization: Bearer CHANGELOG_ADMIN_TOKEN` | Upsert releases by `version`, entries by `(release_id, slug)`, delete entries no longer present in that release, record an `imports` row. Transactional. 401 without token, 422 on parse errors with the offending line. |
| `/api/changelog` | GET `?lang=en&limit=20&cursor=` | none | JSON list of releases with entries, newest first; cached 5 minutes (`Cache-Control: public, max-age=300, stale-while-revalidate=3600`). |
| `/changelog/rss.xml` | GET | none | RSS 2.0 built with `@astrojs/rss` from the same query. |
| `/changelog.json` | GET | none | JSON Feed 1.1 (nice for aggregators). |

Rate limit the import endpoint by token only (it is not public); no other write surface exists.

### 2.5 Pages

- `web/src/pages/changelog/index.astro` (`prerender = false`): header/footer from `SiteHeader`/`SiteFooter` (today `changelog.astro` duplicates the header inline, `changelog.astro:8-12`; use the components). Layout: a left timeline of versions with dates, entries grouped by category with coloured chips (Added coral, Fixed green, Changed blue, Limitation muted), tag filter chips (`#files`, `#ai`, `#launcher`…) driven by query string so the page stays server-rendered and crawlable, "Subscribe" links to RSS and the GitHub releases page, and a fallback block "Changelog is temporarily unavailable; read it on GitHub" when the DB query fails (never a 500).
- `web/src/pages/changelog/[version].astro` (`prerender = false`): one release per URL (`/changelog/0.1.0/`) with its own title/description/OG for SEO and sharing.
- Spanish: `/es/changelog/` renders the same data; entries are English-only at first (`lang` column deferred; note it as a known limitation on the ES page).
- JSON-LD: `SoftwareApplication` `releaseNotes` on the version page.

### 2.6 Push script (`scripts/changelog/push.ts`, run with `tsx`)

Parse `CHANGELOG.md` → `{ releases: [{ version, releasedOn, entries: [{ category, title, bodyMd, docsUrl, tags, position }] }] }`. Validate: unknown header → error with line number; a bullet outside a `###` group → error; duplicate version → error. `--dry-run` prints the JSON. Vitest tests: fixture changelog → expected JSON; malformed fixtures → exact error messages; an import against the compose Postgres is idempotent (two pushes, same row counts, `updated_at` unchanged when content is identical).

### 2.7 Tests and checks for part 2

- Vitest: parser (6 cases), import upsert/delete semantics (3 cases), API auth (401/200/422), RSS validity (`feedvalidator` schema or a snapshot).
- Build-time: `/changelog/` is excluded from prerender; the sitemap includes `/changelog/` and every `/changelog/<version>/` through a custom `sitemap.customPages` built from a DB query at build time only when `DATABASE_URL` is set, else from `CHANGELOG.md` headers (so a build without DB still produces a complete sitemap).
- Manual: push the restructured `CHANGELOG.md`, open `/changelog/`, compare entry counts with `grep -c '^- ' CHANGELOG.md`.

### 2.8 Follow-ups (not in scope now)

- Per-entry Spanish bodies.
- In-app "What's new" reading `/api/changelog`.
- Download counters or a Homebrew analytics panel (the DB is ready for more tables; do not add them now).

## Risk

Risk: item 5 (delivery/environment: new hosting, DNS, container, CI workflow replaced) and item 1 (database writes; mitigated by the single import path, transactions and Coolify backups). Verification per the master plan §7; the verifier checks the production URLs, not only the local build.
