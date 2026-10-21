<!-- Produced by the parent session (read-only exploration, no code changed). To be executed by Sonnet. Evidence is cited as path:line; anything not observed is marked UNVERIFIED. -->

# Release pipeline and in-app updates: execution plan

User request (verbatim, L1): "ahora vas a elaborar un plan que ejecutara sonnet en el que se compile la app se suba como release a github (ademas de la instalacion de brew) y ademas haremos que la app se auto actualice cuando haya una version estable nueva, tambien desde el menu de opciones se debe poder parar actualizaciones o buscar nuevas actualizaciones que no salgan automstico  como en cualquier programa".

Decision D1 (user, L2): releases stay **ad-hoc signed for now, prepared for Developer ID later**. Consequences accepted: after a self-update macOS asks for Accessibility again (TCC anchors ad-hoc apps to the binary hash), and a DMG downloaded by a browser needs "Open Anyway" on Sequoia. The in-app updater downloads with `curl`, which sets no quarantine flag, so the self-update itself is not blocked. The pipeline reads the signing identity and notary profile from secrets when present and falls back to ad-hoc, so switching later is a secrets change, not code.

Goal: `scripts/release.sh X.Y.Z` tags a stable version; GitHub Actions builds, signs (ad-hoc or Developer ID), packages, publishes a GitHub Release with the DMG, an update ZIP and checksums, and refreshes the Homebrew formula. The running app checks that release feed, offers or installs the new stable version by itself, and Settings lets the user turn automatic checks or installs off and check by hand. Brew installs are never self-replaced: they are told to run `brew upgrade ferrocast`.

---

## 1. Current state (evidence)

| What | Where | Note |
|---|---|---|
| Version | `Cargo.toml:6` (`[workspace.package] version = "0.1.0"`), read by `scripts/bundle.sh:8` and shown in About (`draw.rs:5031`) | One source; `CARGO_PKG_VERSION` at runtime. |
| Build/package scripts | `scripts/bundle.sh` (ad-hoc or `FERROCAST_SIGN_IDENTITY`), `scripts/dmg.sh`, `scripts/notarize.sh` (needs `NOTARY_PROFILE`), `scripts/update_formula.sh OWNER/REPO [version]` | All exist and are documented; nothing calls them from CI except `bundle.sh` in `ci.yml`. |
| CI | `.github/workflows/ci.yml` (fmt, clippy, test, bundle on macos-14, Rust 1.95.0), `changelog-push.yml` (web changelog on CHANGELOG.md push), `web-check.yml` | No release workflow, no tag trigger. |
| Homebrew | `Formula/ferrocast.rb` pins `v0.1.0` source tarball + sha256; compiles with `scripts/bundle.sh` on the user's Mac. Docs: `web/src/content/docs/docs/getting-started/install.md`. | Formula bump is manual via `update_formula.sh`. |
| Existing update check | `app.rs:3602 check_updates` behind `Builtin::CheckUpdates` ("Check for Updates", keywords at `items.rs:186`) | Brew-only: runs `brew outdated`; non-brew installs get "update from the repository". No network check of GitHub. |
| Tray menu | `services.rs:591-602 make_tray`, handled at `app.rs:2195` (`open`, `clipboard`, `settings`, `quit`) | Easy to add `check_updates`. |
| Network | `curl` via `std::process::Command` (`services.rs:735`, `821`, `842`), 15 s timeout pattern | No HTTP crate; keep curl. |
| Hashing | `sha2 = "0.11.1"` in `crates/core/Cargo.toml:15` | Enough for SHA-256 of the download. |
| Settings tabs | `draw.rs:5164` (General, Clipboard, Snippets, AI, Scripts, Advanced); `launch_at_login` row at `draw.rs:5244` | Updates section goes in **General** under "Launch at login" (user said "menú de opciones"). |
| Background threads | `services.rs:281/334/454` started from `app.rs:914-916` | Same pattern for the update poller. |
| Single instance | `crates/app/src/instance.rs:15 acquire` (flock on `ferrocast.lock`) | Relaunch must happen after the old process releases the lock (exit first, then `open`). |
| CHANGELOG | `## [Unreleased]` section at `CHANGELOG.md:7`, `## [0.1.0] - 2026-10-15` | Keep-a-changelog shape; release notes can be cut from it. |

**UNVERIFIED**: GitHub repo settings (branch protection on `master`, whether `GITHUB_TOKEN` may push a formula commit; if protected, the workflow opens a PR instead), macos-14 runner having `hdiutil`/`codesign` (it does on GitHub-hosted runners, but confirm in the first run), and that the Accessibility grant really survives an ad-hoc rebuild as `install.md:27` hopes (the plan assumes it does **not** and documents the re-grant).

---

## 2. Decisions

- **D1 Signing**: ad-hoc now; secrets `APPLE_SIGN_IDENTITY_P12`, `APPLE_SIGN_IDENTITY_PASSWORD`, `APPLE_NOTARY_*` optional. Workflow step "Sign and notarize" runs only when they exist.
- **D2 Artifacts per release** (tag `vX.Y.Z`): `Ferrocast-X.Y.Z.dmg` (humans), `Ferrocast-X.Y.Z.zip` (updater; `ditto -c -k --keepParent dist/Ferrocast.app`), `SHA256SUMS.txt`. The updater never uses the DMG.
- **D3 Feed**: GitHub Releases API `https://api.github.com/repos/drumst0ck/ferrocast/releases/latest` (returns only the latest non-draft, non-prerelease release = "stable"). Tags with a suffix (`v1.2.0-beta.1`) are published as prereleases by the workflow and therefore never offered. No custom appcast, no server.
- **D4 Integrity**: the ZIP's SHA-256 must match `SHA256SUMS.txt` from the same release (both over HTTPS from GitHub). No signature key in v1; a minisign/ed25519 key is the Developer-ID-era follow-up and is listed, not built.
- **D5 Channels**: `brew` when the running bundle path contains `/Cellar/` (`std::env::current_exe`, `services.rs:1442` pattern) → never self-replace; "Update" means run `brew upgrade ferrocast` in a thread and relaunch. `direct` otherwise → self-replace. `dev` when the bundle is under `target/` or `dist/` of a git checkout → checks are allowed, installs are refused with a toast.
- **D6 Settings** (General tab, "Updates" group): `update_check: bool` (default true; off = never touch the network), `update_install: Automatic | AskFirst` (default **AskFirst**, because the re-grant of Accessibility under D1 is disruptive; flip the default to Automatic when Developer ID lands), "Check now" button with the result inline ("Ferrocast 0.1.0 is up to date", "0.2.0 available · Install", "Checked 2 h ago"). Plus tray item "Check for Updates…" and the existing `Builtin::CheckUpdates` command rerouted to the new checker.
- **D7 Cadence**: one check 30 s after launch and every 24 h while running, only when `update_check` is on. A failed check is silent (logged); only a manual check toasts errors.
- **D8 Install flow (direct)**: download ZIP to `~/Library/Application Support/Ferrocast/updates/<version>/`, verify SHA-256, `ditto -x -k` into that folder, `codesign --verify --deep --strict` the new bundle, swap: rename current `Ferrocast.app` → `Ferrocast.app.previous`, move new into place, then spawn a detached `sh -c 'sleep 1; open -n "<path>"'` and exit(0). On the next launch delete `.previous` and the download folder. If the current bundle's parent is not writable (e.g. `/Applications` owned by another user), toast "Cannot write to <dir>; download the DMG" with a button that opens the release page.
- **D9 Ask-first UX**: a toast is not enough for a choice. Use the existing `View::About`-style single pane: new `View::Update { version, notes, state }` with the release notes (markdown body of the release, first 40 lines), buttons "Install and relaunch", "Later", "Skip this version" (`update_skipped: Option<String>`). Automatic mode installs without the view and shows one toast "Updating to X.Y.Z…".
- **D10 Release script**: `scripts/release.sh X.Y.Z` bumps `Cargo.toml` + `Cargo.lock`, moves `## [Unreleased]` into `## [X.Y.Z] - YYYY-MM-DD`, commits `release: vX.Y.Z`, tags `vX.Y.Z`, pushes branch and tag. Refuses a dirty tree, a non-master branch, or a tag that exists.

---

## 3. Units

Common rules: RED test first with its exit code recorded, then implement; closing checks as three separate commands with individual exit codes (`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`); no process spawned on the UI thread; no network in tests (feed parsing and planning are pure, the network step is a thin function). Commit tag `RU<n>: <what>`. Work on branch `feat/release-and-updates`.

### RU1. Release workflow — M, verifiable only by a real tag (H1)
Files: `.github/workflows/release.yml`, `scripts/bundle.sh` (small), `scripts/release.sh` (new), `scripts/release_notes.sh` (new).
- `release.yml` on `push: tags: ['v*']`, runs-on `macos-14`, Rust 1.95.0 (same as `ci.yml`):
  1. fmt, clippy, test `--locked` (reuse the steps; do not skip them on tags).
  2. Assert the tag equals `v$(Cargo.toml version)`; fail otherwise.
  3. Optional signing: if `secrets.APPLE_SIGN_IDENTITY_P12` is set, import it into a temporary keychain and export `FERROCAST_SIGN_IDENTITY`; else leave ad-hoc. `bundle.sh` already honours the variable (`bundle.sh:9`).
  4. `scripts/bundle.sh`, `scripts/dmg.sh`, `ditto -c -k --keepParent dist/Ferrocast.app dist/Ferrocast-X.Y.Z.zip`, `shasum -a 256 dist/*.dmg dist/*.zip > dist/SHA256SUMS.txt`.
  5. Optional notarization: when `APPLE_NOTARY_KEY_ID/ISSUER/KEY` exist, `xcrun notarytool submit` the DMG and the ZIP and staple the DMG (the ZIP's app is stapled before zipping: staple `dist/Ferrocast.app`, then re-zip).
  6. Release notes: `scripts/release_notes.sh X.Y.Z` prints the `## [X.Y.Z]` section of `CHANGELOG.md` (fallback: "See CHANGELOG.md"). Prerelease flag = tag contains `-`.
  7. `softprops/action-gh-release@v2` with the three files, `prerelease` as above, `fail_on_unmatched_files: true`.
  8. Formula: `scripts/update_formula.sh drumst0ck/ferrocast X.Y.Z` (the tag's tarball exists by now), then commit `chore(brew): ferrocast X.Y.Z` to `master` with `GITHUB_TOKEN`; if the push is rejected (branch protection, **UNVERIFIED**), open a PR with `peter-evans/create-pull-request@v6` instead. Skip for prereleases.
- `scripts/release.sh` per D10, with a `--dry-run` that prints what it would do. Test: a bash test in `scripts/tests/release_dry_run.sh` run by `ci.yml` (it only needs git and sed) that bumps a scratch clone and asserts the CHANGELOG heading, the commit message and the tag name. RED: the script does not exist (exit 127).
- `ci.yml`: add the scripts test; keep the rest.
- Docs: `web/src/content/docs/docs/contributing/` gets `releasing.md` (how to cut a release, where the secrets go, what happens without them) and `install.md` gains a "Download the DMG" section with the Sequoia "Open Anyway" note under D1; `es/` mirrors.

### RU2. Core: update feed, version compare, plan — S, model-verifiable
New `crates/core/src/update.rs`.
- `Version` parse/compare for `X.Y.Z` with optional `-pre` (pre < release; `v` prefix tolerated). Tests: ordering table, garbage → None.
- `Release { version, tag, zip_url, sums_url, notes, html_url }` from the `/releases/latest` JSON (`serde_json::Value`, no new crate): picks the asset named `Ferrocast-<ver>.zip` and `SHA256SUMS.txt`; draft/prerelease → `None`. Tests on a fixture JSON string (two assets, missing asset, prerelease).
- `sum_for(sums_txt, file_name) -> Option<String>` (shasum format). Test.
- `Channel { Brew, Direct, Dev }` and `channel_of(exe_path: &Path) -> Channel` (`/Cellar/` → Brew; `/target/` or `/dist/` → Dev; else Direct). Tests.
- `Decision` = `UpToDate | Available(Release) | Skipped(Release)` from `(current, latest, skipped)`; `Plan { Download(zip, sum) | BrewUpgrade | Refuse(reason) }` from `(Channel, writable_parent: bool)`. Tests cover every arm, including "newer than latest" (dev build) → `UpToDate`.
- `due(last_check: Option<i64>, now: i64) -> bool` for the 24 h cadence. Test.

### RU3. App: checker, installer, relaunch — M, model-verifiable + H2-H5
Files: `services.rs`, `app.rs`, `models.rs`, `instance.rs`.
- `models.rs`: `update_check: bool` (true), `update_install: UpdateInstall { Automatic, AskFirst }` (AskFirst), `update_skipped: Option<String>`, `update_last_check: Option<i64>` (unix secs), all with serde defaults.
- `services::update_check(bus, manual: bool)`: thread → `curl -sf -m 15 -H "Accept: application/vnd.github+json" -H "User-Agent: Ferrocast/<ver>"` the feed → `Msg::UpdateChecked { manual, result: Result<Option<Release>, String> }`. Rate limit: GitHub allows 60 unauthenticated requests/hour per IP; one per day is far under it.
- `services::update_install(release, bus)`: thread → download ZIP and sums with `curl -sfL -m 600 -o`, verify SHA-256 (`sha2`), `ditto -x -k`, `codesign --verify --deep --strict`, D8 swap, → `Msg::UpdateInstalled { path }` or `Msg::UpdateFailed(String)`. Each external step has a deadline and its stderr first line is the error. Never touch the running bundle before the new one is verified.
- `services::brew_upgrade(bus)`: `brew upgrade ferrocast` with `HOMEBREW_NO_AUTO_UPDATE=1`, up to 20 min, → relaunch message.
- `app.rs`: `start_update_poller` (30 s after launch, every 24 h, only when `update_check`), `Msg::UpdateChecked` handling per D6/D9 (Automatic → install straight away unless Dev or Skipped; AskFirst → `View::Update`), `Msg::UpdateInstalled` → detached `open -n` + `exit(0)`, cleanup of `.previous` and the download dir at startup (`instance.rs` or `App::new`). `Builtin::CheckUpdates` and tray `check_updates` call the manual check; the old brew-only code moves into the Brew channel branch.
- Tests (`tests.rs`, RED first): manual check with `Ok(None)` toasts "up to date"; `Available` in AskFirst opens `View::Update` with the version and both buttons; `Available` with `update_skipped == version` does nothing; Automatic on `Channel::Dev` toasts the refusal; `UpdateFailed` toasts and leaves the view; the 24 h `due` gate is honoured (no `Msg` sent when `update_check` is off: assert the poller thread is not started by checking the shared flag). The real download/swap is H3-H5.

### RU4. Settings and About — S, screenshot
- General tab, group "Updates" after "Launch at login" (`draw.rs:5244`): toggle "Check for updates automatically", segmented "Install updates: Ask first / Automatically", row with "Version X.Y.Z · Checked 2 h ago / never" and a "Check now" button; when a release is available the row shows "X.Y.Z available" and an "Install" button; Brew channel shows "Installed with Homebrew: updates run brew upgrade"; Dev channel shows "Development build: checks only".
- About view (`draw.rs:5020`): a "Check for Updates…" button next to GitHub.
- Tray: "Check for Updates…" item before the separator (`services.rs:594`).
- Tests: render both themes with each channel forced through a test hook; toggling the check off clears `update_last_check` display; settings round-trip.

### RU5. Docs and changelog — S
- `CHANGELOG.md` Unreleased: release pipeline, in-app updates, settings.
- `web/src/content/docs/docs/launcher/settings.md` (Updates group), `getting-started/install.md` (DMG download, Open Anyway, how brew users update, what the app does by itself), new `contributing/releasing.md`; `es/` mirrors. README/README.es: one line under Install ("Download the DMG from Releases, or brew…").

---

## 4. Order, size, risk

RU2 → RU3 → RU4 → RU5 → RU1 last (so the first real tag exercises everything, including the updater reading its own release). ~900 authored lines (RU1 ~250 incl. workflow and scripts, RU2 ~250, RU3 ~300, RU4 ~120, RU5 docs). Four to six commits; RU1 and RU3 each get their own commit.

Risk line: **item 5** (delivery: CI workflow, release artifacts, formula bump) and **item 1** (the installer renames and replaces the running bundle; mitigated by verify-before-swap, `.previous` kept until the next successful launch, and refusal on Brew/Dev channels). Both are named high-risk items → independent verification applies to RU1 and RU3 (`gentle-ai-verify` over the diff; the live parts stay human).

## 5. Sonnet guardrails

- No HTTP crate, no Sparkle, no new signing keys: curl + sha2 only.
- Never modify the running bundle before the new one passed SHA-256 and `codesign --verify`; never delete `.previous` in the same run.
- Brew and Dev channels never self-replace. The brew-only logic of `check_updates` (`app.rs:3602`) is moved, not duplicated.
- `update_check = false` means zero network calls: assert it in a test, and keep the poller thread unstarted rather than idle.
- Secrets are optional at every step; the workflow must succeed with none set (ad-hoc path). Do not print secrets or keychain passwords in logs.
- Do not push a tag or create a release while implementing; the first tag is the user's call (H1).
- Record each check's exit code separately.

## 6. Human checklist

- **H1** Push `v0.1.1` (or a `v0.1.1-rc.1` prerelease first, which the app must ignore): workflow green, release has DMG, ZIP, SHA256SUMS; formula commit or PR landed; `brew upgrade ferrocast` works from the tap.
- **H2** Direct install (`/Applications/Ferrocast.app` from the DMG, past "Open Anyway"): Settings → Check now reports the newer version; AskFirst shows the Update view with notes.
- **H3** Install and relaunch: the new version starts, `.previous` is gone after the next launch, Accessibility re-grant prompt appears (expected under D1) and the wheel/expansion work after re-granting.
- **H4** Automatic mode on a Mac that has been running 24 h+: the update applies with one toast and relaunches.
- **H5** Brew channel: the Updates group says Homebrew; "Install" runs `brew upgrade` and relaunches.
- **H6** `update_check` off: no entry in the GitHub API log for a day (use `tcpdump`/Little Snitch or trust the test).
- **H7** Failure paths: airplane mode → manual check toasts an error; corrupt ZIP (edit SHA256SUMS in a local mock) → "checksum mismatch", bundle untouched.
