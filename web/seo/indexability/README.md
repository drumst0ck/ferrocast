# Conversion indexability review — 2026-11-02

Repetitive content is an unconfirmed hypothesis; recovery is not guaranteed. No quota was used.

284 English conversion URLs: 275 pairs + 9 hubs. 113 pairs are protected by the union of clicks/impressions in the supplied exports, without summing overlapping windows. Five recently improved guides are among those 113. Each pair's actual rendered branches were reviewed: authored guides, pair-specific limitations and curated bilingual pairs are conservative quality exceptions. Generic pages repeat four wheel steps and FAQs, changing format labels/tool prerequisites. Exclude only these repetitive pages with no observed demand, never for zero impressions alone.

Result: 123 pairs kept (113 demand + 10 quality exceptions), 152 noindex, 9 hubs kept. English conversion sitemap entries: 284 before, 132 after. decisions.csv records every decision, reason, and per-source signals. sources/ preserves the original exports and live sitemap. JSON windows: 18–24 and 25–31 October; baseline CSV exported 28 October.

All bilingual English counterparts remain indexable, preserving reciprocal hreflang groups. No Spanish or other-family indexability changes; no robots.txt, navigation or canonical changes.

## Reverse

Set enabled=false in src/data/convert-indexability.json, rebuild and deploy to restore all entries and remove noindex. Or remove an individual exact path to restore just that URL. Both HTML and sitemap use this manifest.

## Verify

npm test; npm run build; node seo/indexability/verify.mjs. Artifact verification covers all 284 URLs (robots, canonical, content and sitemap), other-family samples and every generated hreflang target. Verify production HTTP 200 and deployed metadata after Coolify completes.
