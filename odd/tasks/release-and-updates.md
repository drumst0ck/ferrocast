# Release pipeline and in-app updates
Branch: feat/release-and-updates (stacked on feat/translate-wheel) · Plan: odd/plans/release-and-auto-update-plan.md · Status: in progress

## Specs
- S1 "se compile la app se suba como release a github (ademas de la instalacion de brew)": tag vX.Y.Z builds, packages (DMG, ZIP, SHA256SUMS), publishes a GitHub Release and refreshes the Homebrew formula.
- S2 "la app se auto actualice cuando haya una version estable nueva": only stable (non-prerelease) releases; brew installs are never self-replaced.
- S3 "desde el menu de opciones se debe poder parar actualizaciones o buscar nuevas actualizaciones que no salgan automatico": Settings toggle for automatic checks, install mode, "Check now"; tray and About entries.
- S4 D1 (user): ad-hoc signing now, Developer ID later via optional secrets.

## Tasks
- T1 S2,S3 RU2 core update.rs (version, feed parse, channel, decision, plan, due) — commit: pending
- T2 S2,S3 RU3 app checker/installer/relaunch + settings fields — commit: pending
- T3 S3 RU4 Settings Updates group, About button, tray item — commit: pending
- T4 S1-S3 RU5 docs + changelog — commit: pending
- T5 S1,S4 RU1 release.yml, release.sh, notes script, ci test, docs — commit: pending

## Log
- L1 "ahora vas a elaborar un plan que ejecutara sonnet en el que se compile la app se suba como release a github (ademas de la instalacion de brew) y ademas haremos que la app se auto actualice cuando haya una version estable nueva, tambien desde el menu de opciones se debe poder parar actualizaciones o buscar nuevas actualizaciones que no salgan automstico  como en cualquier programa"
- L2 decision: "Ad-hoc por ahora, preparado para Developer ID después"
- L3 "ejecuta el plan"
