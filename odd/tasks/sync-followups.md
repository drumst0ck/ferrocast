# Annotated follow-ups after the Mejoras panel
Branch: feat/sync-followups · Status: in progress
Close the improvements left open in odd/tasks/icloud-sync.md (L13), odd/tasks/custom-themes.md and odd/tasks/window-tiling.md.

## Specs
- S1 User (L1, verbatim): "borra todo lo que ya este fusionado, haz las mejoras anotadas".
- S2 Custom themes sync with iCloud like the other synced data: themes.json becomes a sync domain (per-theme merge, deletions propagate as tombstones), off when iCloud sync is off; the chosen theme follows the settings sync rules already in place.
- S3 iCloud residual N2 (icloud-sync.md L13): "a tracker lost right after a merge that was offered but not applied can turn the older local value into a new edit; rare, follow-up: write the merged state to the own document only once applied".
- S4 iCloud residual N1 (icloud-sync.md L13): "documents written by earlier builds of this branch count every field as edited; no user has them, feature unreleased" — decide whether any action is needed now that the feature is released.
- S5 Window tiling not yet checked with Stage Manager on real hardware (window-tiling.md); a second display is not available on this Mac.

## Tasks
- T1 S2 themes.json sync domain (core sync + app wiring + docs en/es). — pending
- T2 S3 N2 fix, test-first. — pending
- T3 S4 N1 decision recorded. — pending
- T4 S5 Live Stage Manager check of tiling (pause notice, resume) on this Mac; second display reported as not testable here. — pending
- T5 Independent verification (risk item 1: synced user data), PR, merge. — pending

## Log
- L1 User (verbatim): "borra todo lo que ya este fusionado, haz las mejoras anotadas". Cleanup done first: 5 worktrees, 27 local and 42 remote branches removed after checking each was merged (ancestor of master, merged PR, or same commits merged under another branch: feat/seo-convertir-es and feat/seo-launcher-compare via #54/#55; feat/raycast-extensions via #82-#85).
