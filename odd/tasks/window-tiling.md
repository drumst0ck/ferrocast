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
- T1 S3 core: pure layout engine (tests): layouts → frames for N windows on a screen rect with gaps; split-line moves for minimum sizes; float when it cannot fit; stack when full; focus-mode save/restore. — pending
- T2 S3 macOS: window observation (AX notifications for created/destroyed/moved/focused windows), filter dialogs/sheets/panels, apply frames, per-display layout, Stage Manager detection, min-size learning store (local). — pending
- T3 S3,S4 app: Settings → Windows: tiling on/off (off by default), layout, gap ("Space"), highlight border on/off + colour, activate on hover, per-app "Tile" exclusions, pause switch in the menu bar menu; launcher commands; hotkeys ⌃⌥⌘1–4, ⌃⌥Z, ⌃⌥+arrows across displays. — pending
- T4 S3 quick actions above windows (small overlay bar on the focused tiled window). — pending
- T5 docs (en/es), CHANGELOG, tests. — pending
- T6 verify: real windows on one and two displays, exclusions, dialogs, focus mode restore, pause. — pending

## Log
- L1 Evidence: Notion idea "Copiar todas las funciones" (Media); shojimac.com read on 2026-10-27 (features quoted in S3). Ferrocast today: crates/macos/src/windows.rs has 40 one-shot commands via Accessibility; no observers, no persistent layout.
- L2 User (verbatim): "si me vale, no creo que se pueda sinceonizar permisos del mac pero todo lo demas que se copie del 2 punto copia todo"
- L3 Risk: item 4 (concurrency: AX observers and the UI thread) and item 6 (hard to test window behaviour automatically) → independent verifier.
