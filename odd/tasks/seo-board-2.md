# SEO board, second round (priority 2)
Branch: feat/seo-board-2 · Status: pages built, pending parent review
Pages from the Notion "Ferrocast · SEO/AEO / Tablero de ideas" board created on 2026-10-28, priority 2. Cards and briefs are copied verbatim under odd/tasks/seo-board-2/.

## Specs
- S1 User (L1, verbatim): "comprueba tareas del notion" → chose "Todo, por orden de prioridad".
- S2 Rules carried from odd/tasks/seo-notion-ideas.md (verbatim user constraints): never invent facts or metrics; competitor facts only from official sources; do not write "no tiene" without verification; no rival speed or RAM claims; FAQ JSON-LD must equal the visible text; docs descriptions 120–155 characters.
- S3 Each page follows its card (odd/tasks/seo-board-2/card-*.md) and, when present, its brief (brief-*.md): target terms, intent, honest limits named in the card.
- S4 iCloud sync is not in a release before 0.4.0; 0.4.0 is published (2026-10-28), so pages may say sync is available from 0.4.0.

## Tasks
- T1 S3 /clipboard-manager-mac (EN) — brief-clipboard.md. — done, commit 344e0ae
- T2 S3 /text-expander-mac + /es/expansor-de-texto-mac — brief-text-expander.md. — done, commit 131d538
- T3 S3 /cleanshot-alternative (EN) — brief-cleanshot.md. — done, commit c3c12cd
- T4 S3 /es/editar-video-mac-sin-imovie — card-editar-video-idea.md. — done, commit e8d8dc4
- T5 S3 /es/gestion-de-ventanas-mac — card-gestion-ventanas-idea.md. — done, commit e531c4f
- T6 S3 /es/ia-local-mac — card-ia-local-idea.md. — done, commit bff570a
- T7 S3 /es/captura-de-pantalla-con-scroll-mac — card-captura-scroll-idea.md. — done, commit fb237c5

## Log
- L1 User (verbatim): "comprueba tareas del notion"; chose "Todo, por orden de prioridad". Same session: the priority-3 idea /lightweight-mac-launcher was skipped ("Saltarla").
- L2 T1-T7 built (one commit each, plus 90a30ae linking /alfred-alternative/ to T1/T2). Sync is described as available from 0.4.0 (S4): the briefs' "coming in the next version" wording is superseded, and /es/historial-portapapeles-mac/ was corrected (history leaves the Mac only with Sync with iCloud on). Competitor facts re-read on 2026-10-28: maccy.app (MIT, macOS 14+, password-manager line, App Store listing $9.99), pasteapp.io/pricing, Apple Support (Spotlight clipboard, Text Replacement, window tiling, screenshot guide), textexpander.com/pricing (Free 5 active snippets, Pro $60/yr, nested snippets and date math from Pro), manual.raycast.com/snippets, espanso.org (forms, GPL-3), cleanshot.com/pricing and /features, rectangleapp.com (now states Free and Open Source, macOS 14+; /mac-window-management/ still says its licence is not stated), magnet.crowdcafe.com (price not stated). Dropped: Paste Lifetime price and password behaviour, Spotlight retention times (not on Apple's page), Maccy Intel and OCR, TextExpander/Espanso import, CleanShot Intel, horizontal scroll and QR/language claims for Ferrocast. Checks: vitest all green except sitemap-entries.test.ts, which counts web/CHANGELOG.md releases and fails after `npm run build` because scripts/sync-changelog.mjs refreshes that tracked file (outside this task's surfaces) to include 0.4.0; build ok; each new page has title/description, canonical, hreflang pair for EN+ES, HowTo/FAQPage/BreadcrumbList with every FAQ answer present in the visible HTML.
