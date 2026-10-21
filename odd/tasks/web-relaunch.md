# Web relaunch: ferrocast.app (infra, changelog DB, docs, home, SEO, project info)
Branch chain: chain/16..21 off master · Owner: parent session · Status: in progress
Plans: odd/plans/web-relaunch-master-plan.md (read first) + five sibling plans.

## Specs
- S1 Domain `https://ferrocast.app`; Coolify + Docker; PostgreSQL; Starlight at /docs; EN default + ES under /es/ with hreflang (master plan §1, L2).
- S2 Astro static + @astrojs/node standalone, changelog/API routes on demand, /healthz, base '/' (deploy plan §1.1-1.2).
- S3 CHANGELOG.md is the source of truth; pushed to PostgreSQL by scripts/changelog/push.ts via token-protected import endpoint; /changelog, /changelog/<version>, RSS, JSON feed (deploy plan §2).
- S4 Docs site Starlight, ~55 pages EN, ES for priority pages, generated reference from conversions.json/tools.json (docs plan).
- S5 Home redesign covering file tools, Raycast audience and file audience; i18n; honesty vocabulary (home plan).
- S6 SEO: sitemap, hreflang, JSON-LD, 275 /convert pages + 26 /tools pages, /compare/raycast, llms.txt, analytics (seo plan).
- S7 README EN + ES, CHANGELOG Keep a Changelog, CONTRIBUTING, repo metadata (project-info plan).
- S8 Honesty: every claim traces to README or CHANGELOG; never upgrade Built to Tested.

## Tasks
- T1 (S1,S2) chain/16-web-infra: Dockerfile, node adapter, origin change, healthz, www redirect, CI, redirect shell. Route: inline. Commit: see git log (chain/16-web-infra)
- T2 (S3) chain/17-changelog-db: schema, parser, API, pages, RSS. Route: inline. Commit: pending
- T3 (S4) chain/18-docs-site: Starlight, IA, generated pages. Route: inline/worker. Commit: pending
- T4 (S5) chain/19-home-redesign. Route: inline. Commit: pending
- T5 (S6) chain/20-seo. Route: inline. Commit: pending
- T6 (S7) chain/21-project-info. Route: inline. Commit: pending
- T7 External (S1) Coolify app+DB, DNS, GitHub repo metadata: needs owner credentials/consent. Status: pending

## Log
- L1 owner: see odd/plans/web-relaunch-master-plan.md Log L1 (verbatim). "ejecutalo"
