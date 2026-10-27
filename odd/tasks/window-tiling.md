# Automatic window tiling (Shoji features)
Branch: feat/window-tiling · Status: in progress
Bring the features Shoji (shojimac.com) lists into Ferrocast's window manager: automatic, always-on tiling with layouts, focus mode, per-display layouts and the customisation options, next to the existing 40 one-shot window commands.

## Specs
- S1 Notion "Mejoras Ferrocast / Copiar todas las funciones" (verbatim): "Copiar todas las funciones de https://shojimac.com/ para mejorar nuestro gestor de ventanas"
- S2 User (verbatim, L2): "del 2 punto copia todo" — scope is every feature below, not a subset.
- S3 Features from shojimac.com (read 2026-10-27), as Shoji words them:
  - "Open a window and Shoji gives it a place. No dragging, no resizing, no layouts to set up." (automatic tiling of every new window)
  - "You choose the layout. Shoji does the arranging." — "Picked in Shoji’s menu, or with ⌃⌥⌘1–4." (four layouts, menu + shortcuts)
  - "Focus mode: One window fills the screen. Press it again and every window goes back exactly where it was." — ⌃⌥Z
  - "Borders, spacing, colors, even the icon." — Highlight, Color, "Space · 8 pt"
  - "Quick actions above windows", "Activate windows on hover", "Keyboard shortcuts", "Stack when the screen is full", "Open at login"
  - "Each display gets its own layout, and ⌃⌥ + arrow past the edge of a screen sends the active window to the next one."
  - "Can I keep an app out of the layout? … turn off “Tile” for it. Dialogs, sheets and panels are never tiled."
  - "Shoji learns the smallest size an app accepts, moves the split lines to make room, and floats the window in the middle when it can’t fit. That list stays on your Mac."
  - "Stage Manager … Shoji pauses while it’s on and picks up again when you turn it off."
  - "Can I pause it? Any time, with the switch in the menu-bar panel. Your windows stay where they are."
  - Privacy: works with each window's position, size and app only; "window titles are never saved".
- S4 Off by default: tiling moves the user's windows, so it starts only when turned on in Settings (an ordinary, reversible choice; Ferrocast already has "Open at login").
- S5 Do not copy Shoji's text, images or icon; implement the behaviour.

## Tasks
- T1 S3 core: pure layout engine (tests): layouts → frames for N windows on a screen rect with gaps; split-line moves for minimum sizes; float when it cannot fit; stack when full; focus-mode save/restore. — done (L4)
- T2 S3 macOS: window observation (AX notifications for created/destroyed/moved/focused windows), filter dialogs/sheets/panels, apply frames, per-display layout, Stage Manager detection, min-size learning store (local). — done, polling instead of AXObserver, not run against real windows (L4)
- T3 S3,S4 app: Settings → Windows: tiling on/off (off by default), layout, gap ("Space"), highlight border on/off + colour, activate on hover, per-app "Tile" exclusions, pause switch in the menu bar menu; launcher commands; hotkeys ⌃⌥⌘1–4, ⌃⌥Z, ⌃⌥+arrows across displays. — done except the border overlay (L4)
- T4 S3 quick actions above windows (small overlay bar on the focused tiled window). — pending
- T5 docs (en/es), CHANGELOG, tests. — done (L4)
- T6 verify: real windows on one and two displays, exclusions, dialogs, focus mode restore, pause. — pending

## Log
- L1 Evidence: Notion idea "Copiar todas las funciones" (Media); shojimac.com read on 2026-10-27 (features quoted in S3). Ferrocast today: crates/macos/src/windows.rs has 40 one-shot commands via Accessibility; no observers, no persistent layout.
- L2 User (verbatim): "si me vale, no creo que se pueda sinceonizar permisos del mac pero todo lo demas que se copie del 2 punto copia todo"
- L3 Risk: item 4 (concurrency: AX observers and the UI thread) and item 6 (hard to test window behaviour automatically) → independent verifier.
- L4 slice 1 (T1, T2, T3). Pure engine `crates/core/src/tiling.rs` (734 lines, 25 tests); macOS worker `crates/macos/src/tiling.rs` (626 lines, 3 tests); app wiring in app.rs/draw.rs/items.rs/services.rs/models.rs/settings_index.rs; docs en+es `launcher/tiling.md`; CHANGELOG.
  - RED: tests written first; with the engine bodies stubbed `cargo test -p ferrocast-core --lib tiling` gave 22 failed / 3 passed. GREEN: 25 passed with the implementation.
  - Tests cover: each of 4 layouts, gap, one window, min-size split-line moves (`distribute`, `a_learned_minimum_moves_the_split_line`), float when it cannot fit, stack when full (cascade in last slot), capacity, focus save/restore exactness, neighbour/next-display, multi-display independence, MinSizes learn + JSON (no titles), action ids, default hotkeys. App: tiling off by default / no worker, commands listed, 40 window commands intact, pause/toggle, hotkey defaults, tray item; settings reset/index tests updated.
  - Results: `cargo test -p ferrocast-core` 795 passed; `cargo test -p ferrocast` 963 passed, 36 ignored; `cargo clippy --all-targets` no warnings; `cargo fmt --all --check` clean.
  - Design: S4 — worker is only spawned when tiling or hover is on; tiling hotkeys registered only while tiling is on. Polling every 400 ms (AXObserver needs a run loop and per-app re-subscription; polling covers launch/quit/create/close/user-move with one path). Worker owns all AX calls; it talks to the app only via `Event` → `Msg::Tiling` on the bus. Min sizes persisted by the app (`tiling-min-sizes.json`, keyed by bundle id, never titles). Stage Manager read from `com.apple.WindowManager` `GloballyEnabled`.
  - Not covered / next slice: T4 quick actions bar; highlight border overlay (settings stored, TODO(T4) in draw.rs); T6 real-window verification (nothing was run against real windows); "Open at login" already exists.
  - Uncertain: screens are read by hopping to the main queue with `dispatch_sync_f` (fine while the run loop spins); hover raise ignores windows covering the pointer that are not tracked; apps with a hung AX server are cut off by a 0.25 s messaging timeout; per-display layouts are keyed by display index, so they shift if displays are reordered; Settings lives on the Advanced tab (no new tab, to keep tab numbers).
