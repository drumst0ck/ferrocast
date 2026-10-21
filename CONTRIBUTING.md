# Contributing

## Setup

- macOS on Apple Silicon. Rust 1.95 is pinned in `rust-toolchain.toml` (egui 0.36 requires it).
- `cargo test --workspace` must pass. CI also runs `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings`.

## Layout

| Crate | Purpose |
|---|---|
| `crates/core` | Platform-independent logic: search, calculator, placeholders, scripts, storage. Pure and unit tested. |
| `crates/macos` | AppKit, Accessibility, CoreGraphics and pasteboard bindings. |
| `crates/app` | The egui launcher, background services and UI-state tests. |

## Working on the UI without screen permission

`scripts/shot.sh out.png FERROCAST_VIEW=settings` renders the window to a PNG from inside egui.
Supported inputs: `FERROCAST_VIEW` (clipboard, snippets, quicklinks, emoji, settings, notes, ai,
welcome, shortcuts, about), `FERROCAST_QUERY`, `FERROCAST_PANEL=1`. Use a throwaway
`FERROCAST_DATA_DIR` so real data is never touched.

## Branches and pull requests

- Work on a topic branch and open small, chained pull requests (`chain/NN-topic`), about 400 changed lines each, with the tests and docs that belong to the behavior in the same slice.
- Use Conventional Commit messages (`feat(web): …`, `fix(tools): …`).
- Do not claim a feature is Tested unless you ran it (on real files or in a real session); describe in the PR how you checked anything that needs Accessibility or a live session. The record is `docs/STATUS.es.md`.

## Website, docs and changelog

The site lives in `web/` (Astro, Starlight docs, PostgreSQL changelog) and deploys to ferrocast.app through Coolify.

- `cd web && npm ci && npm run dev` starts it. `npm run build` also checks that the generated data still holds 275 conversion pairs over 46 inputs.
- `docker compose up -d db` in `web/` starts PostgreSQL for the changelog (copy `.env.example` to `.env`). `npm test` runs the unit tests; set `TEST_DATABASE_URL=postgres://ferrocast:ferrocast@localhost:5432/ferrocast` to include the database tests.
- After changing conversions or tools in Rust, regenerate the site data with `cargo run -p ferrocast-core --example site_data` and commit `web/src/data/*.json`.
- Every user-visible change needs an entry in `CHANGELOG.md` under `[Unreleased]`: one bullet per change in a `###` category (`Added`, `Changed`, `Fixed`, `Removed`, `Security`, `Known limitations`), an optional `**Title**:` prefix, optional trailing `#tags` and `(docs: /docs/…)`. Check it with `cd web && npm run changelog:push -- --dry-run`. A push to `master` that touches `CHANGELOG.md` publishes it.
- A new feature also needs its documentation page under `web/src/content/docs/`.

## Rules of thumb

- Put logic in `core` with tests; keep `app` thin.
- Anything destructive must go through the confirmation view.
- Never put secrets in argv or logs. API keys live in the Keychain.
- Hotkeys, pasting, focus and snippet expansion need Accessibility and cannot run in CI: describe
  how you checked them by hand in the pull request.

## Releasing (Homebrew, no paid Apple account)

1. Bump `version` in the root `Cargo.toml`, move `[Unreleased]` in `CHANGELOG.md` to a dated `## [X.Y.Z] - YYYY-MM-DD` section, make sure CI is green.
2. `git tag vX.Y.Z && git push --tags`.
3. `scripts/update_formula.sh drumst0ck/ferrocast` rewrites `Formula/ferrocast.rb` (url + sha256) from the pushed tag; commit and push it.
4. Users run `brew upgrade ferrocast`.

The formula compiles from source on the user's machine, so the app has no quarantine flag and needs no
Developer ID. `scripts/notarize.sh` exists only for anyone who has a paid account and wants a signed DMG.
