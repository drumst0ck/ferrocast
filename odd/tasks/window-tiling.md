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
- L5 fix round on f2d0baa (verifier findings 1-10 + notes). Fixes in `crates/core/src/tiling.rs` (pure), `crates/macos/src/tiling.rs` (worker), `crates/app/src/{app,services}.rs` (wiring), tests in core/macos/`crates/app/src/tests.rs`, docs en/es.
  - 1 display unplugged: `reconcile_displays` (core) used by `Worker::sync`; test `displays_going_away_keep_every_window`. RED observed (stub: 9 core tests failed before implementation).
  - 2 false minimums: `refusal`/`should_learn` (only first read after our own arrange, never in focus mode, position kept, larger size), `MinSizes::learn_capped` (capped by display), `MinSizes::confirm` (lowers a stored minimum an app later accepted); worker clears applied/verify on focus enter/restore. Tests `only_a_refusal_of_our_own_frame_teaches_a_minimum`, `learned_minimums_are_capped_by_the_display_and_decay_when_an_app_accepts_less`. RED observed.
  - 3 worker stops: `sync_tiling` drops the handle when tiling and hover are both off (channel disconnect ends the worker); `tick` does not enumerate while paused/off unless hover is on. Test `toggle_and_pause_use_the_spawn_seam_and_the_worker_stops_when_everything_is_off` (RED not observed: needs the new seam).
  - 4 AX timeout: `bound()` on app, every window element, focused window and raise; `survivors` keeps ids of apps whose list call failed. Test `windows_of_an_app_that_timed_out_keep_their_ids` (macos; written with the helper, RED not observed).
  - 5 refused position: `user_moved(prev, now, target, applied_since_read)`; test `a_refused_position_is_not_a_user_move_but_a_real_drag_is`. RED observed.
  - 6 hover: `window_stack()` (CGWindowList front to back) + `top_window_at` + `Hover` state (raises again after leaving). Tests `hover_raises_after_the_dwell_and_again_after_leaving`, `hover_uses_the_window_on_top_not_the_first_that_contains_the_pointer`. RED observed.
  - 7 cascade: `tile` floats stacked windows whose learned minimum does not fit the cascade frame. Test `a_stacked_window_whose_minimum_does_not_fit_the_cascade_floats`. RED observed.
  - 8 hotkeys: `services::plan_hotkeys` (launcher, user bindings, user tiling bindings, then colliding-free tiling defaults only while tiling is on; user `tiling:toggle` kept while off; skipped ones reported through the hotkey error toast). Tests `tiling_defaults_are_not_registered_while_tiling_is_off_or_on_top_of_the_users_shortcuts`, `a_users_own_tiling_toggle_shortcut_is_kept_while_tiling_is_off`. RED observed (stubbed plan).
  - 9 seam: `Effects.tiling_spawn`; test fake records `tiling spawn` and returns a `Handle::from_sender`; tests `the_default_fixture_never_spawns_the_tiling_worker`, `the_worker_is_told_a_clamped_gap`.
  - 10 `clamp_gap` (0-64, NaN to 0) and `valid_screen`; `tile` returns no frames for a bad screen; worker filters invalid screens and sends a clamped gap. Tests `gap_is_clamped_and_screens_validated`, `an_absurd_gap_or_a_bad_screen_never_puts_frames_off_screen`. RED observed.
  - Notes: focus mode returns early while Stage Manager is on (and re-reads the preference); `truthy` reads CFBoolean or CFNumber (test `a_stage_manager_preference_can_be_a_boolean_or_a_number`); tray item is `Pause Tiling`/`Resume Tiling` by state and only while tiling is on (`tray_menu_items(recording, tiling)`, test `the_tray_offers_pause_or_resume_by_state_and_only_while_tiling_is_on`).
  - Results: `cargo test -p ferrocast-core` 804 passed (tiling 34); `cargo test -p ferrocast-macos` 248 passed, 10 ignored (tiling 5); `cargo test -p ferrocast` 967 passed, 36 ignored; `cargo clippy --all-targets` no warnings; `cargo fmt --all --check` clean.
  - Still open: T4, border overlay, T6 on real windows; none of the worker changes were run against real windows; stale wrong minimums only decay through `confirm` (no timed decay).
- L6 re-verification fixes on ca16fce. Core `crates/core/src/tiling.rs`, worker `crates/macos/src/tiling.rs`, `crates/macos/src/windows.rs`.
  - 1 pause/Stage Manager keep state: `Worker::idle` keeps tracked windows, order and focus mode while paused or under Stage Manager (only enumeration/arranging stop; state is dropped only when tiling is off); `was_idle` makes the first sync after resuming skip user-move detection. Test `pause_and_stage_manager_keep_the_order_and_focus_mode` (RED observed against the old clearing behaviour: order was `[]`), `turning_tiling_off_drops_the_tracked_state`.
  - 2 duplicate windows with 3+ displays: window display now comes from the list it is in (`display_index`); `move_to_display` removes the id from every list before adding it, used by the sync, move and arrow paths; `reconcile_displays` also drops duplicate ids. Test `a_window_is_never_listed_twice_after_a_display_goes_away` (displays 0,1,2, unplug 2, move: exactly once). RED observed.
  - 3 async resize: `confirm_refusal` learns only a refusal repeated on two consecutive reads (the worker re-applies each tick until `MAX_TRIES`); transient sizes that settle teach nothing. Test `a_refusal_must_repeat_before_it_is_learned`. RED observed.
  - 4 `windows::focused_window` bounds the app and the returned window with the 0.25 s timeout. No deterministic test (needs a hung app); `AxElement::drop` now ignores a null pointer so worker-state tests can use placeholder windows.
  - 5 floats: each further floating window is offset by `CASCADE_STEP` and clamped inside the area. Test `several_floating_windows_do_not_sit_exactly_on_top_of_each_other`. RED observed.
  - Still open: T4, border overlay, T6 on real windows (the worker was never run against real windows).

