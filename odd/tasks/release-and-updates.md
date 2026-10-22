# Release pipeline and in-app updates
Branch: feat/release-and-updates (stacked on feat/translate-wheel) · Plan: odd/plans/release-and-auto-update-plan.md · Status: in progress

## Specs
- S1 "se compile la app se suba como release a github (ademas de la instalacion de brew)": tag vX.Y.Z builds, packages (DMG, ZIP, SHA256SUMS), publishes a GitHub Release and refreshes the Homebrew formula.
- S2 "la app se auto actualice cuando haya una version estable nueva": only stable (non-prerelease) releases; brew installs are never self-replaced.
- S3 "desde el menu de opciones se debe poder parar actualizaciones o buscar nuevas actualizaciones que no salgan automatico": Settings toggle for automatic checks, install mode, "Check now"; tray and About entries.
- S4 D1 (user): ad-hoc signing now, Developer ID later via optional secrets.

## Tasks
- T1 S2,S3 RU2 core update.rs (version, feed parse, channel, decision, plan, due) — commit: b72f0a7
- T2 S2,S3 RU3 app checker/installer/relaunch + settings fields — commit: 3662da0
- T3 S3 RU4 Settings Updates group, About button, tray item — commit: 9e75d86
- T4 S1-S3 RU5 docs + changelog — commit: a1e14c2
- T5 S1,S4 RU1 release.yml, release.sh, notes script, ci test, docs — commit: b69d68d

## Log
- L1 "ahora vas a elaborar un plan que ejecutara sonnet en el que se compile la app se suba como release a github (ademas de la instalacion de brew) y ademas haremos que la app se auto actualice cuando haya una version estable nueva, tambien desde el menu de opciones se debe poder parar actualizaciones o buscar nuevas actualizaciones que no salgan automstico  como en cualquier programa"
- L2 decision: "Ad-hoc por ahora, preparado para Developer ID después"
- L3 "ejecuta el plan"

- L4 Slice boundary: everything on this branch sits on top of feat/translate-wheel (stacked); RU1 (b69d68d) is its own commit as a high-risk unit. Delegated to gentle-ai-worker: RU2, RU3, RU4 (each re-checked by the parent: fmt/clippy/workspace exit 0). Final counts: 307 app, 340 core, 52 macos tests green; scripts/tests/release_dry_run.sh ok (RED: macOS sed lacks `0,/re/`, fixed with awk).
- L5 Independent verification: gentle-ai-verify TIMED OUT (stalled 30 min on a "guarded git push" prompt) and produced no verdict; reported as unavailable, not retried. Parent read install_blocking (services.rs:874-919): SHA-256 and codesign both pass before the first rename of the running bundle, rollback of .previous on a failed placement, relaunch passes the path as $1. Repo state checked after the stall: no new tag, clean tree, remote heads unchanged (nothing was pushed).
- L6 NOT proven (needs real runs): H1 a real tag run of release.yml (including the formula PR and that GITHUB_TOKEN may open PRs), H2-H5 download/swap/relaunch on a DMG install, brew channel, automatic mode after 24 h, H6 no network when update_check is off, H7 failure paths. Settings/About layout not seen on screen. Web docs not built. Risk: item 1 (bundle replacement) and item 5 (CI/release/formula).

- L7 Native review (lineage review-42c8326bce502dbb, lenses risk/resilience/reliability, base 72bfaf3, 30 files): approved after ONE bounded correction (commit 12902c2, 47 diff lines of the 60 declared): `brew upgrade` exited 0 without updating when the tap was stale or the formula PR was unmerged, so the app relaunched the same version; now `brew update` runs first and the app relaunches only if `brew list --versions` changed (test brew_upgrade_tests, RED 101 then green). Authority acknowledged and burned. Workspace after the fix: 308 app, 340 core, 52 macos tests green, fmt/clippy 0.
- L8 Advisory findings (non-blocking, follow-up work, no re-review): R1-001 release.yml:27-31 and R1-002/003 services.rs:884-902,823 (risk); R3-channel-symlink-path update.rs:120-129 (a symlinked exe path may misclassify the channel); R3-poller-not-started-on-enable services.rs:850-853 (turning "check automatically" ON after launch needs a restart; the poller is not started when the app launched with it off); R3-install-swap-untested; R4-concurrent-install-no-guard services.rs:880-915 (two installs can race); R3-translate-stale-paste / R4-late-translation-paste app.rs:2130-2141 (a slow translation pastes into whatever app is frontmost when it finishes). The last two belong to the translate wheel and are worth fixing before relying on Replace mode.
