# iCloud sync between Macs
Branch: feat/icloud-sync · Status: in progress (T2 done)
Sync Ferrocast's data between the user's Macs through iCloud Drive, opt-in, merging per item so no Mac loses another Mac's changes.

## Specs
- S1 Notion "Mejoras Ferrocast / Sincronizacion con icloud" (verbatim): "sincronizar todos los ajustes chats portapapeles etc etc con icloud para poder sincronizar toda la app entre diferentes macs"
- S2 User decision (L2), what syncs: "Ajustes, snippets y quicklinks", "Notas", "Chats de Ask AI", "Historial del portapapeles".
- S3 User decision (L2), conflicts: "Fusionar por elemento" — "Cada snippet, nota o entrada gana por su fecha; no se pierden elementos nuevos de ningún Mac."
- S4 User decision (L2), activation: "Desactivada; se activa en Ajustes" — "Nadie sube datos a iCloud sin elegirlo."
- S6 User decision (L6), local-only settings (verbatim): "si me vale, no creo que se pueda sinceonizar permisos del mac" — approving the proposal: script folders, Obsidian vault, launch at login, capture camera and microphone devices, update preferences and state, and UI state stay on each Mac; everything else syncs, hotkeys included. macOS permissions (Accessibility, Screen Recording, etc.) are per Mac and are not data Ferrocast can sync.
- S5 Constraint (stated to the user in the question): Keychain API keys do not sync (the app is ad-hoc signed; iCloud Keychain and CloudKit need Apple entitlements).

## Design (proposed)
- Transport: the iCloud Drive folder `~/Library/Mobile Documents/com~apple~CloudDocs/Ferrocast/` (a plain folder iCloud Drive syncs; no entitlements). If it does not exist, the setting explains that iCloud Drive must be on.
- No shared file is written by two Macs: each Mac writes only `devices/<device-id>/<domain>.json`; every Mac reads all device folders and merges. This avoids iCloud conflict copies.
- Per-item last-writer-wins by `modified` with tombstones so deletions propagate. The times live in the sync layer, not in the models: a local tracker (`sync-state`) keeps each item's content hash and time, so only real local edits get a new time and items received from another Mac are not echoed back. Existing data files keep their format.
- Settings: one item per field group; machine-specific fields (hotkeys, script folders, window gap?) stay local — list to confirm in T1.
- Ask AI chats are not persisted today (in memory only): T-chat persists them first.
- Clipboard images (`clipboard-images/`) sync as files referenced by entries; the sync respects the retention setting and never syncs entries the password filter skipped (they are never stored).
- Pick-up: watch the folder (FSEvents) plus a periodic check; apply merges on the main thread.

## Tasks
- T1 S6 decided (L6). — done
- T2 S3 core: merge engine (pure, tested): per-item LWW + tombstones, device folders, deterministic tie-break by device id. — route: inline — commit: see L5
- T3 dropped: the tracker in T2 keeps times outside the models, so no data format changes.
- T4 S2 persist Ask AI chats (chats.json). — pending
- T5 S4 Settings → Sync tab: toggle (off by default), status, last sync, iCloud Drive missing message. — pending
- T6 S2 app wiring: write own device file on save, watch + merge, clipboard images. — pending
- T7 docs (en/es), CHANGELOG, website copy only where true. — pending
- T8 verify: two data dirs simulating two Macs (FERROCAST_DATA_DIR + sync dir override), concurrent edits, deletions, old-format files. — pending

## Log
- L1 User (verbatim): "mientras esperas puedes seguir con el panel de notion de ideas" then "mira el panel de mejoras"
- L2 User answers: icon "Icono a color de la app"; iCloud data: all four; conflicts "Fusionar por elemento"; activation "Desactivada; se activa en Ajustes".
- L3 Evidence: crates/core/src/storage.rs writes JSON files atomically in Application Support (settings, snippets, quicklinks, notes, clipboard, frecency, calc-history…); Snippet/Quicklink/ClipEntry have no modified time; Note has created/modified/deleted_at; Ask AI conversation is not persisted.
- L4 Risk: item 1 (rewriting stored data, format change) and item 4 (concurrency) → independent verifier before merge.
- L5 T2 done: crates/core/src/sync.rs (publish, winners, merge, Tracker; tombstones dropped after 90 days) with 12 tests covering both-new items kept, newer remote edit wins without echo, unpublished local edit wins, remote/local deletions, resurrection by a later edit, tie-break by device, duplicates, JSON round trip. Tests written together with the code (no separate RED run). ferrocast-core 782 passed, clippy clean for sync.rs. Not wired into the app yet.
- L6 User (verbatim): "si me vale, no creo que se pueda sinceonizar permisos del mac pero todo lo demas que se copie del 2 punto copia todo" (first part answers T1; the second answers the Shoji scope, tracked in odd/tasks/window-tiling.md).
