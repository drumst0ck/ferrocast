# Enter on Finder selection: risk note

Status: built, opt-in, default OFF. Not proven in a real session (see "Manual check H3").

## What it does

With **Settings > General > Enter on Finder selection** on, Ferrocast installs a
system-wide keyboard event tap (a default, not listen-only, `CGEventTap` at Session level, key-down only).
When the user presses a plain Return in Finder with a file selection focused, the tap
swallows that key press and Ferrocast opens the Convert wheel for the selection. Finder
therefore does **not** start a rename on Return while the setting is on (this is the point
of the feature, and the price of it).

If the selection is empty, cannot be read, or has no conversions, Ferrocast posts the
Return back, marked so the tap never intercepts it again, and Finder behaves as usual.

With the setting off there is no tap, no workspace observer and no Accessibility prompt.

## What it can break

- A tap sits in the path of **every** key press. A tap callback that stalls can freeze or
  delay keyboard input system-wide. macOS disables a tap that is too slow
  (`kCGEventTapDisabledByTimeout`); input then returns to normal, but the feature stops
  until the tap is re-enabled.
- A bug in the decision logic could swallow Return where it should not (a text field, another app).
- A swallowed Return that is never given back would be a lost key press.
- While the tap is on, the process-wide Accessibility messaging timeout is 50 ms
  (`AXUIElementSetMessagingTimeout` on the system-wide element is global). Other Ferrocast
  Accessibility calls (window management) share it and can fail on a very slow app.
  It is set only after the tap exists and restored by a drop guard (`AxTimeoutGuard`) on every
  exit path, including early errors and panics of the tap thread.
- The first Return with the feature on may trigger a macOS Finder **Automation** (osascript)
  permission prompt. Until it is answered the selection read waits.
- The selection read can take up to 15 s. During that time the Return is neither handled nor
  re-posted, and a late re-post can start a rename 15 s later.
- If the user switches apps during the read, the Return is silently lost (the re-post returns false
  because Finder is no longer frontmost).
- Only the key-down is dropped: the matching key-up of the swallowed Return still reaches Finder.
- A **held** Return: the first press is swallowed, the auto-repeats pass through to Finder, which
  starts the rename the feature was meant to avoid.
- If Accessibility is missing the setting stays on in the UI (a toast explains); no tap exists.
- Turning the setting off joins the tap thread on the UI thread, up to about 0.5 s.
- Rename by Return in Finder stops working while the setting is on (rename with a slow
  click on the name, or turn the setting off).

## Safeguards

1. **Off by default.** `Settings.enter_on_finder` is false; old `settings.json` files load as false.
2. **Narrow interception.** The tap swallows Return only if all hold: setting on, Finder is the
   frontmost app (tracked by an NSWorkspace activation observer, read as an atomic), the key is
   a key-down that is not an auto-repeat, keycode 36, no shift/control/option/command, the
   focused element role is one of AXList, AXOutline, AXBrowser, AXScrollArea, AXImage, and the
   focused element belongs to Finder's process (pid read with `AXUIElementGetPid` must equal the pid
   kept by the activation observer; defense in depth if the cached "Finder frontmost" flag is
   stale; unknown pid passes through), and the focused element sits in a window with subrole AXStandardWindow (so sheets, dialogs and
   Quick Look panels pass through). Text fields, search fields, rename edits, unknown or
   missing roles pass through. The rule lives in `crates/core/src/finder_enter.rs` and is table-tested.
   Anything the tap cannot classify passes through. A Save dialog belongs to the other app, so Finder is not frontmost; a
   Finder dialog is not a standard window.
3. **O(1) callback.** Atomics, at most four Accessibility reads (focused element, its role, its window,
   the window's subrole; the owner pid read is local) with a 50 ms timeout each, so about 200 ms in
   the worst case for a plain Return in Finder (only after the cheap checks pass), and one channel send to a helper thread. No locks, no file or
   process I/O, no blocking calls. The selection read (osascript, 15 s deadline) runs off the UI thread.
4. **Disabled-tap recovery.** On `TapDisabledByTimeout` / `TapDisabledByUserInput` the tap is
   re-enabled from the callback, and again every 2 s by the tap thread (covers sleep/wake).
5. **No Return loop.** The re-posted Return carries a dedicated marker
   (`REPOST_MARKER`, user-data field); the tap always lets it through. In addition the tap
   passes every Return through for 300 ms after a re-post (a new re-post restarts the window; pure and tested in `InFlight`), and while one Return is being
   handled a second one is passed through, never queued.
6. **Permission.** Turning the setting on asks for Accessibility once. If it is not granted,
   or the tap cannot be created, no tap exists, the setting stays as set, and a toast says
   "Enter on Finder needs Accessibility access". No retry loop.
7. **Clean off switch.** Turning the setting off drops the tap: the callback goes inert at
   once, the run loop stops, the thread is joined, the observer is removed and the
   messaging timeout is restored. No restart needed.

## If something goes wrong

1. Turn the setting off (Settings > General > Enter on Finder selection). The tap is removed immediately.
2. If the keyboard misbehaves and Ferrocast's window is not reachable: quit Ferrocast (Activity Monitor or `killall ferrocast`). The tap dies with the process.
3. Remove Ferrocast from **System Settings > Privacy & Security > Accessibility**. Without it the tap cannot exist.

## Not tested automatically

The tap itself, the activation observer, the Accessibility reads, sleep/wake recovery, Quick Look,
a Finder Save dialog, several keyboards and the real behaviour of macOS timeouts need a real
session. The automatic tests cover only the pure rules, the marker, the settings, the app
handlers with injected hooks, and the settings toggle. No test posts real key events.

## Manual check H3 (plan line ~568)

With Accessibility granted and "Enter on Finder selection" **on**:

- Select a file in Finder and press Return: the Convert wheel opens.
- Select nothing (click empty space) and press Return: Finder behaves as usual, with no loop.
- Return in the Finder search field and in a rename edit (after slow-click on a name) is untouched.
- Cmd+Return, Option+Return, Shift+Return, Control+Return and holding Return are untouched.
- Quick Look (Space) and a Save dialog in another app are unaffected.
- Typing speed does not degrade (type in a text editor while the setting is on).
- Put the Mac to sleep and wake it: Return on a selection still opens the wheel.

With the setting **off**: Return on a selected file renames it, as before, and the Ferrocast process has
no event tap (nothing in Accessibility use, no prompt).
