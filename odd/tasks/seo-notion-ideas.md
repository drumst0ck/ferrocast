# SEO ideas from Notion (Ferrocast · SEO/AEO)
Branch: feat/seo-notion-ideas · Status: in progress
Work through the Notion board "Ferrocast · SEO/AEO / Tablero de ideas" (19 ideas, priority 1 first). Source of truth for each page is its Notion brief; ideas without a brief get one before the page.

## Specs
- S1 User (verbatim, L1): "alli hay ideas para hacer de seo, empezamos a hacerlas"
- S2 Notion brief "Brief: Alternativa a Raycast (ES) · /es/alternativa-a-raycast/" (page 4063a474adbb81ca9f2cdf3dabe036f2), verbatim: URL `https://ferrocast.app/es/alternativa-a-raycast/`; title `Alternativa gratis a Raycast para Mac, de código abierto | Ferrocast`; meta `Ferrocast es un lanzador gratis y MIT para Mac: Script Commands de Raycast, IA local con Ollama y 275 conversiones sin subir archivos. Lo que falta, también.`; H1 `Una alternativa a Raycast gratis y de código abierto para Mac`; "redirección 301 desde /es/alternativa-raycast/"; hreflang es ↔ en with x-default on the EN page; sections 5 (H2 outline), 6 (table), 7 (10 FAQ + SoftwareApplication, FAQPage, HowTo, BreadcrumbList JSON-LD), 9 (internal links, llms.txt).
- S3 Brief rules, verbatim: "Lo marcado ⚠️ no se pudo verificar en fuente oficial: no se publica hasta comprobarlo."; "No poner tiempos de arranque ni consumo de memoria de Raycast o Alfred"; "decir que Raycast o Alfred «no tienen» algo que solo no hemos verificado" is forbidden; "Sin aggregateRating"; "El texto debe coincidir exactamente con el visible" (FAQPage); "Español neutro de España, de tú".
- S4 Brief section 10, verbatim: "/raycast-alternative/ (idea #1). Misma estructura y la misma tabla con las mismas fuentes; hreflang recíproco." and "No es una traducción literal ... El ángulo EN debe apoyarse más en «launcher + conversión local + grabación en una sola app de 8,5 MB»." Board idea #1 title: "Free, open-source Raycast alternative for Mac (with file conversion and screen recording)".

## Tasks
- T1 S1-S3 `/es/alternativa-a-raycast/` page + redirect + llms.txt + sitemap lastmod + entry links (home footer en/es, SiteFooter, migration guides en/es, /facts, llms.txt). — route: inline — commit: see L4
- T2 S4 `/raycast-alternative/` EN sister page sharing the component and verified table; hreflang pair. — route: inline (same component as T1) — commit: see L4
- T3 Board P1 "Compress a video to an exact size on Mac (Discord 10/25 MB, email) – offline" EN+ES: brief first. — pending
- T4 Board P2 "Raycast vs Alfred vs Spotlight (macOS Tahoe) vs Ferrocast" ES. — pending
- T5 Board P2 hub `/es/convertir` (HEIC→JPG, PDF→Word, MOV→MP4, vídeo→GIF, WebP→JPG). — pending
- T6 Board P2 "Run Raycast Script Commands without Raycast" EN. — pending
- T7 Board P2 "Ferrocast vs Raycast" EN. — pending
- T8 Board P2 AEO hub (README, /llms.txt, /facts already shipped in #41; remaining gaps only). — pending
- T9 Board P3-P5 (11 ideas): Alfred alternative, PDF→Word ES/EN, HEIC guide, Screen Studio alternative EN/ES, local AI launcher, Permute/CloudConvert alternative, clipboard ES, mov-to-gif, window management. — pending

## Log
- L1 User (verbatim): "Te conecte el mcp de notion, alli hay ideas para hacer de seo, empezamos a hacerlas"
- L2 Evidence: Notion board has 19 ideas; only "/es/alternativa-raycast" has state "brief hecho" (brief at /es/alternativa-a-raycast/). Local master was 11 commits behind origin (facts.astro, llms.txt rewrite, sitemap lastmod, README.es Raycast y Alfred section already landed); pulled before branching. Live: /facts/ 200, /es/facts/ 404, /raycast-alternative/ 404.
- L3 Decision: competitor cells marked ⚠️ in the brief are dropped or written as "no consta en su web" (S3); competitor facts carry the brief's check date 27-oct-2026 and official source links.
- L4 T1+T2 done: copy in web/src/lib/seo/alt-raycast.ts (+ test), web/src/components/seo/AlternativePage.astro, pages es/alternativa-a-raycast.astro and raycast-alternative.astro; astro `redirects` gives a real 301 from /es/alternativa-raycast/ on the node server; sitemap lastmod mapping; SiteFooter gains lang + link. Checks: vitest 46 passed, build ok, local server 301/200, canonical + hreflang en/es/x-default, JSON-LD SoftwareApplication/FAQPage(10)/HowTo/BreadcrumbList with every FAQ answer present in visible HTML, no horizontal overflow at 1280 and 390, no console errors. Verdict: S1 started, S2 done (title/meta/H1/URL/redirect/schema/links verbatim), S3 done (⚠️ cells written as "no consta en su web"/dropped; no rival speed/RAM), S4 done (EN angle "three apps in one 8.5 MB launcher"). Not verified: Rich Results test on production, Safari. Notion state not changed (no deploy yet).
