<!-- Produced by claude-fable via el Gentleman (planning only). To be executed by Sonnet. Read odd/plans/web-relaunch-master-plan.md first. -->

# README, CHANGELOG, CONTRIBUTING, in-app and repository metadata refresh

Branch `chain/21-project-info`. Runs last so every link points to pages that exist.

## 1. README

Today: `README.md` in Spanish, 17.8 KB, dominated by a 50-row status table with verification notes. That table is precious (it is the honesty record) but it is not what a first-time visitor needs. The table moves to `/docs/reference/status/` (docs plan §2) and the README becomes a short English landing with a Spanish twin.

### 1.1 `README.md` (English, ≤ 250 lines)

```
<p align="center"><img src="web/public/icon.png" width="96"></p>
# Ferrocast
Fast, open-source launcher for macOS, written in Rust. Search, calculate, paste and ask AI from the keyboard; drag any file, tap a key, and convert, compress, crop, trim or redact it on your Mac.
[badges: CI · License MIT · Homebrew tap · Website · Docs · Changelog]
[hero: web/public/img/home.jpg and files-wheel-convert.jpg side by side, or one GIF ≤ 2 MB]

## Install          ← the exact brew block from web/src/data/install.ts (a script checks parity)
## What it does     ← 10 bullets, one per capability, each linking to its docs page
## File tools       ← 5 bullets: 275 conversions / 26 tools / 7 actions / drag + Shift/Command/Control / local, never overwrites
## Ask AI           ← providers in one paragraph incl. the ChatGPT sign-in caveat sentence
## Status and honesty ← 1 paragraph: what "Tested/Built/Typed" mean + link to /docs/reference/status/
## Comparison       ← 3 lines + link to /compare/raycast/; trademark note
## Build from source ← scripts/bundle.sh, dmg.sh, cargo test, rust 1.95, Apple Silicon; link to docs contributing
## Documentation    ← ferrocast.app/docs, changelog, RSS
## Privacy          ← data dir, JSON unencrypted, Keychain, nothing uploaded
## License
```

Rules: every claim comes from the old README rows (keep a mapping in the PR description); the "Por qué una fórmula y no un cask" paragraph survives translated (it answers the most common objection); the "Verificado" and "Permisos al actualizar" notes move to docs install/troubleshooting; the "Qué está verificado y qué no" section becomes the Status paragraph plus the docs page.

### 1.2 `README.es.md`

Same structure in neutral Spanish; a language link at the top of both files. `scripts/check_readme.sh` (or a Vitest test in `web/`) asserts both contain the identical install block and the same set of headings count.

### 1.3 Keep in the repo

`docs/RESEARCH.md` (feature research) stays and gets a header "historical; see ferrocast.app/docs". `docs/IMPLEMENTATION.md` is superseded: fold its decisions into `/docs/contributing/architecture/` and delete the file (or leave a 5-line pointer). `odd/` is working material; add `odd/README.md` saying so, so visitors do not mistake it for docs.

## 2. CHANGELOG.md

Restructure to Keep a Changelog as specified in `changelog-db-and-deploy-plan.md` §2.2. Concretely:

- `## [0.1.0] - <date of the first public tag>`; if there is no tag yet, keep `## [Unreleased]` and move everything under it, then cut `0.1.0` when the Homebrew formula is bumped (`scripts/update_formula.sh`).
- Split the current mega-bullets into one-feature entries with a bold title and ≤ 3 sentences; the detail paragraphs go to the docs tool pages (docs plan §3.2). Preserve every limitation sentence somewhere (docs "Limits" block); none may be dropped.
- Categories: `Added`, `Changed`, `Fixed`, `Known limitations` (the honesty notes), `Security` (the Script Commands argument-visibility note).
- Tags at the end of each bullet (`#files #video`) and optional `(docs: /docs/files/tools/trim-video/)`.
- Add a top link to `https://ferrocast.app/changelog/` and the RSS feed.

## 3. CONTRIBUTING.md

Update with: branch model (chained PRs `chain/NN-topic`, ~400 lines), `cargo fmt/clippy/test` and `scripts/bundle.sh` as CI runs them (`.github/workflows/ci.yml`), `scripts/shot.sh` for screenshots, `cargo run -p ferrocast-core --example site_data` to regenerate `web/src/data/*.json` and the rule that the website build fails when counts drift, how to write a changelog entry (format from §2), how to add a docs page, how to run the web locally (`npm run dev`, `docker compose up` for the DB), and the honesty rule (never upgrade a status without a real-session check).

## 4. In-app information

Search `crates/app/src` for the About / Welcome / Settings → Advanced strings that mention GitHub or the old site (UNVERIFIED whether any URL is embedded). Add or update: "Website ferrocast.app", "Documentation", "Changelog" links on the About screen; "Check for Updates" (README row) could show the latest version from `/api/changelog` later (out of scope now; note as follow-up). Update `Formula/ferrocast.rb` `desc` and `homepage`.

## 5. Repository metadata

- `gh repo edit --description "Fast, open-source launcher for macOS with local file conversion (275 formats) and file tools. Rust + egui." --homepage https://ferrocast.app --add-topic macos,launcher,raycast-alternative,rust,egui,file-converter,heic,ffmpeg,productivity,open-source`
- Social preview image (Settings → Social preview) = new `og.jpg` (uploaded by hand; cannot be scripted).
- GitHub Release `v0.1.0` with the changelog body, once the version is cut; link to it from the home "Changelog teaser".
- `.github/ISSUE_TEMPLATE/` (bug, feature, conversion-problem with fields: input format, output format, optional tool versions) and `PULL_REQUEST_TEMPLATE.md` with the honesty checklist (changelog entry, docs page, status unchanged unless proven).
- `SECURITY.md` (how to report; the Script Commands argument note).

## 6. Checks

- `README.md` and `README.es.md`: all links resolve (`npx markdown-link-check`), install block parity, badges render.
- `CHANGELOG.md` parses with `scripts/changelog/push.ts --dry-run` and the entry count equals the number of bullets.
- `cargo test --workspace` still passes (if any app strings changed, UI snapshot tests may need updating).
- `gh repo view --json description,homepageUrl,repositoryTopics` shows the new values.

## Risk

Risk: none for runtime behaviour if the app is untouched; if About-screen strings change, item 6 does not apply because `scripts/shot.sh` renders can be compared before/after. Keep the app change in its own commit.
