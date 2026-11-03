# Tool docs vs tool pages — 2026-11-03

Each file tool existed twice in the sitemap, both indexable with their own canonical: `/tools/<tool>/` (product page, 27 URLs with the hub) and `/docs/files/tools/<tool>/` (documentation, 26 URLs). That duplication is a hypothesis for the indexing problem, not a confirmed cause; recovery is not guaranteed.

`/tools/<tool>/` is now the only indexable version. Every `/docs/files/tools/<tool>/` page keeps answering 200, stays in the docs sidebar and search, and links to its tool page (and back), but leaves the sitemap and carries no hreflang.

## Rule (seo/tool-docs/review.mjs)

- Move first: the 15 tools that had the generic `/tools/` page now render the docs' own text (description, steps, options, limits) from the docs source, so nothing unique stays only in the doc. The other 10 guides and `/tools/organize-pdf/` already carried it.
- `canonical` to `/tools/<tool>/` when at least 60% of the doc's content words appear on the tool page and every key, typed command and button name in the doc is on it.
- `noindex,follow` (own canonical kept) when the doc has content the tool page lacks. No tool needed it: all 26 are `canonical`.
- The untranslated `/es/docs/files/tools/<tool>/` fallbacks (already `noindex,follow`) now canonical to the same tool page, so there is no canonical chain.

`decisions.csv` has the decision, overlap, facts checked and reason per URL. `sources/search-signals.json` holds the per-doc search signal taken from the exports kept in `../indexability/sources/`; `sources/sitemap-before.txt` is the live sitemap before the change.

## `/docs/files/tools/organize-pdf/`

The only doc with earlier impressions (paginas.csv export of 28 Oct: 16 impressions; JSON windows 18–24 Oct: 9, 25–31 Oct: 23, position 59.8; 0 clicks; mostly «pdf organizer»; windows not summed). Same rule applies: `/tools/organize-pdf/` already lists every key, the `pages 3,1,2r` command, the buttons and the long-PDF limit, so the doc is canonical to it, which moves those queries to the page meant to answer them. The doc stays reachable. Revert by removing only that path from the manifest.

## Sitemap

Before (live sitemap, identical to `sources/sitemap-before.txt`): 301 URLs, of which 26 `/docs/files/tools/`, 27 `/tools/` and 132 `/convert/`. After (local build): 275 URLs, of which 0 `/docs/files/tools/`, 27 `/tools/` and 132 `/convert/`.

## Reverse

Set `enabled=false` in `src/data/tool-docs-indexability.json`, rebuild and deploy: the docs get their own canonical, hreflang and sitemap entries back. Or delete one `entries` item to restore one doc. Sitemap, canonical and robots all read this manifest. The text now shown on the 15 generic tool pages stays; it is only the docs' own copy.

## Verify

`npm test`; `npm run build`; `node seo/tool-docs/verify.mjs` (canonical, robots, hreflang, sitemap, links both ways, ES fallbacks, every hreflang target) and `node seo/indexability/verify.mjs` (conversions unchanged). Then check production after Coolify finishes.
