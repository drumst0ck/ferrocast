# Designed DMG window
Branch: feat/dmg-design · Status: in progress

## Specs
- S1 User (verbatim, L1): "vale mientras tanto, deberiamos hacer que el dmg que se descarga, tenga diseño y estilo como muchos .dmg que te descargas que sale el logo de la app de fondo con flechas para que muevas la app a la carpeta de apps de mac, mira la skill de frontend para darle un diseño chulo y que tenga que ver con el diseño de la web y la app"
- S2 Opening the DMG shows a fixed-size icon-view window with a designed background (web palette #0f0f11 / #19191c / #f0535f, dot grid, coral glow, Stack Sans), the app on the left, an Applications link on the right and an arrow between them; retina-sharp.
- S3 Finder draws the icon labels black in Light appearance and white in Dark: both must stay legible on the background.
- S4 Built the same way locally and in the release workflow, headless (no Finder scripting); without the DMG tool installed scripts/dmg.sh still produces the plain DMG.
- S5 No release is published in this work (user, L2).

## Tasks
- T1 S2,S3 background design (HTML source + 1x/2x PNG), mock with real icons and both label colours — commit: see git log
- T2 S2,S4 scripts/dmg.sh with dmgbuild settings (window, positions, background, volume icon), fallback — commit: see git log
- T3 S4 release.yml installs pinned dmgbuild — commit: see git log

## Log
- L1 (verbatim above).
- L2 User earlier: "no, vamos a preparar una bateria de cambios antes de publicar release".
- L3 Done: resources/dmg/background.html (web palette, dot grid, coral glow, neon-red arrow like the translating effect, Stack Sans caption, mid-grey label chips at luminance 0.18: 4.67:1 with black, 4.50:1 with white Finder labels), rendered by scripts/dmg_background.sh with headless Chrome to background.png 660x400 and @2x 1320x800; resources/dmg/settings.py + scripts/dmg.sh use dmgbuild 1.6.7 (designed window) and fall back to the plain image without it; REQUIRE_DESIGNED_DMG=1 in release.yml, and ci.yml builds the designed DMG too. Checked: build exit 0, hdiutil verify VALID, .DS_Store window {{200,140},{660,400}}, icons (180,170)/(480,170) 128 pt, background alias, volume icon; plain fallback exit 0; REQUIRE without dmgbuild exit 1. User opened the DMG in Finder: "se ve perfecto". Not verified yet: dmgbuild on the GitHub runner (first CI run).
