---
title: "Notch player"
description: "A now-playing pill at the notch for whatever plays on your Mac (browser, Spotify, Music, Podcasts, VLC…): a spinning vinyl with the cover, and on hover a card with the title, progress ring and previous, play/pause and next."
sidebar:
  order: 21
---
The notch player shows what is playing on your Mac in a small black pill: any app that reports to macOS Now Playing, such as a browser tab (YouTube, Spotify Web), Spotify, Music, Podcasts or VLC. On a MacBook with a notch the pill extends the notch with a wing on each side; on any other Mac it is a capsule at the top centre of the screen. It is **off by default**.

Turn it on in Settings → General → Notch player → **Now playing in the notch**. It starts and stops at once, with no restart, and the setting is saved with your other settings.

## What you see

- **Collapsed**: the cover as a small spinning disc on the left and an equalizer on the right while a track plays. The bars rest low while it is paused, and the pill gives a small pulse when a new track starts.
- **Hover**: the pill opens with a springy, liquid animation into a card: a large vinyl record with the cover (the cover fills the disc without stretching, a wide or tall image is cropped around its centre), a ring around it that fills as the song goes, a big bold title, the artist, the name and icon of the app that plays, **previous**, **play/pause** (the icon morphs) and **next**, and the elapsed and remaining time. A blurred glow of the cover tints the card and a soft light of its main colour spills out under it. The record turns while the song plays, coasts to a stop on pause and gives a quick spin when a new track starts. Move the pointer away and the card closes after a short pause. With **Reduce motion** on in System Settings → Accessibility → Display, it opens and closes without animation, the record does not turn, the glow does not drift and the bars stay still.
- **Seek**: click anywhere on the progress bar to jump there, or press and drag along it to scrub. The bar shows a knob, and while you drag a bubble shows the time; the song only jumps when you let go. The bar is easy to hit (about 16 pt tall) and the pointer becomes a hand over it. A track without a duration, such as a live stream, cannot be seeked.
- **Open the app**: click the title or artist (they highlight and show a hand pointer) to bring the app that plays forward; for a browser tab that is the browser.
- The colours come from the cover (its main colour tints the ring, the progress bar and the glow); a track without a cover uses Ferrocast's accent.

It hides by itself when:

- nothing is playing: no app reports media, or a track has been paused for about 45 seconds (it comes back when you press play; it never hides while the pointer is on it);
- an app is full screen on the main display.

## How it works and permissions

Ferrocast reads macOS's Now Playing through **mediaremote-adapter**, a small open-source helper (BSD 3-Clause, see [Third-party notices](https://github.com/drumst0ck/ferrocast/blob/master/THIRD_PARTY_NOTICES.md)) that ships inside the app. It runs through macOS's own `/usr/bin/perl`, because since macOS 15.4 only Apple's tools may read Now Playing directly.

- **No permission prompt.** It does not use Automation, Accessibility or Screen Recording, and it does not touch the network or write the cover to disk: the cover comes from memory.
- It only runs while the setting is on. Turning it off, or quitting Ferrocast, stops the helper at once.
- Previous, play/pause and next are sent to whatever app macOS considers the current player.

## Limits

- **It may stop working after a macOS update.** The helper relies on a private part of macOS that Apple can change. If it keeps failing, Ferrocast stops retrying, shows "Now playing is not available on this version of macOS" once and keeps the pill hidden until you turn the setting off and on again, or update Ferrocast.
- Only what the playing app tells macOS is shown. An app that does not report to Now Playing does not appear, and some apps send no cover or no duration (the progress bar stays empty without a duration).
- Covers bigger than 4 MB or 2048 pixels on a side are not shown (the placeholder appears instead). A pause keeps the cover of the same track.
- If you hide the menu bar and it slides in when the pointer reaches the top edge, the pill stays on top of it. It only hides for a real full-screen app.
- The equalizer is decorative: macOS does not let an app read another app's audio level without Screen Recording permission, which this feature does not ask for.
- A change made in the player shows up almost at once (updates are merged for a fraction of a second).
- With a notch, the pill is anchored to the main display (the one with the menu bar). The panel asks macOS to leave it out of screenshots and screen recordings; some capture tools may still show it.
- On a Mac without a notch the capsule sits over the middle of the menu bar and can cover a menu bar item there.

## Related

- [Settings](/docs/launcher/settings/)
