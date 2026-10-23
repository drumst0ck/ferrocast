# Live test tools

Committed dev tooling to drive the REAL Ferrocast app (real mouse, keys, screenshots) and to read
what happened. Built into `target/dev-live/` (already covered by the `/target` ignore rule).

- `build.sh`: compiles `poster`, `winlist`, `permcheck`, `permask` (only if missing or stale).
- `poster`: real CGEvents: `move X Y`, `click X Y`, `drag X1 Y1 X2 Y2`, `key CODE [cmd+shift+alt+ctrl]`, `type "text"`. Points, top-left origin.
- `winlist [owner]`: id, owner, layer, sharing state, onscreen, bounds.
- `permcheck` / `permask` `screen|ax|input|camera|mic`: read a permission / trigger its prompt.
- `safe.sh`, `gclick.sh`: guarded wrappers (see Safety).
- `start-app.sh` / `stop-app.sh`: run / kill the debug app with an isolated data dir
  (`LIVE_DATA`, default `target/dev-live/app-data`, tour skipped, captures in `target/dev-live/caps|recs`).
  Set `FERROCAST_HELPER` to copy a built helper next to the binary. Output: `target/dev-live/app.out`.

## One-time grants

Grant Screen Recording, Accessibility and Input Monitoring (System Settings > Privacy & Security)
to the app that hosts your shell (terminal, herdr, ...). Children inherit it. Check with
`target/dev-live/permcheck screen|ax|input`; `permask` shows the prompt. Restart the host after granting.

## Safety (earlier incidents forced these)

- Never post keys unless the app is frontmost: use `safe.sh`, never `poster` directly. It prints
  `REFUSED: front is <name>` and exits 3 otherwise. `LIVE_APP` (default `ferrocast`) is a
  case-insensitive pattern for the app name.
- Clicks only inside the app's onscreen windows: use `gclick.sh X Y`. `move` is allowed anywhere, never a click.
- Never start the app with `open`; use `start-app.sh` (nohup, detached).
- Never type or click into the user's terminal.

## Example session

```sh
scripts/dev/live/start-app.sh                 # prints the pid
scripts/dev/live/safe.sh type "capture area"  # only if Ferrocast is frontmost
scripts/dev/live/safe.sh key 36               # Return
screencapture -x target/dev-live/s.png        # then read the PNG
scripts/dev/live/stop-app.sh
```
