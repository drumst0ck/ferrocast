# Ferrocast: estado de la primera versión

Objetivo: un launcher macOS escrito en Rust, inspirado en Raycast, con identidad
propia. No es una réplica completa ni ejecuta las extensiones React de Raycast.

## Trabajo y verificación

- Núcleo y bindings macOS existentes: commits `771be3d` y `af15540`.
- UI y servicios existentes retomados sin rehacerlos: `crates/app/src`.
- Bloqueo de compilación: egui 0.36 exige Rust 1.95. Se instala y se fija
  solo para este repo con `rust-toolchain.toml`, sin cambiar el toolchain global.
- Estado final y límites: ver README.
  arrancar visible y oculto, empaquetar `.app` y documentar límites reales.

## Decisiones

- Nombre provisional: Ferrocast, para no reutilizar la marca ni assets de Raycast.
- UI Rust con egui, no AppKit nativo: los iconos de apps sí vienen de Finder;
  los emoji son monocromos. Prioridad: versión funcional y testeable.
- Primero macOS (DMG/zip universal para Apple Silicon e Intel; Intel sin probar en hardware real). AI, sincronización, marketplace React y paridad
  completa se dejan para iteraciones posteriores.
- Datos locales en JSON. No se debe afirmar que el historial está cifrado.
- Pruebas del arranque no ejercitan apagado, reinicio, borrado ni scripts ajenos.

## Reproductor del notch (opcional)

- Fuente de datos: «Ahora suena» de macOS (cualquier app: navegadores, Spotify, Música,
  Podcasts, VLC…) mediante `ungive/mediaremote-adapter` (BSD-3, vendorizado en
  `third_party/mediaremote-adapter/` con el commit fijado en su README). Desde macOS 15.4
  `mediaremoted` solo atiende a procesos de Apple, así que el ayudante se ejecuta con
  `/usr/bin/perl <script> <framework> stream` (argv, sin shell), que carga
  `MediaRemoteAdapter.framework`. El framework se empaqueta pero NO se enlaza al binario.
  No hay AppleScript, permisos de Automatización, curl ni ficheros temporales (la carátula
  llega en base64 y se decodifica en memoria). Es una API privada: puede romperse con una
  actualización de macOS; tras varios fallos seguidos se deja de reintentar y se avisa una vez.
- Empaquetado: `scripts/bundle.sh` compila el framework con clang (sin cmake) para la
  arquitectura del binario, lo firma antes que la app (de dentro hacia fuera, runtime
  endurecido con identidad Developer ID) y lo deja en `Contents/Frameworks`; el script perl y
  los avisos de terceros van en `Contents/Resources`. En compilaciones debug se busca además en
  `target/mediaremote-adapter/` o en `FERROCAST_MEDIAREMOTE_DIR` (una compilación release solo carga
  la copia de su propio bundle); si no está, la función
  figura como no disponible sin fallar.
- Límites: línea del adaptador ≤ 8 MB, carátula ≤ 4 MB y ≤ 2048 px por lado (si no, se ignora).
  Animación: muelle abierto/cerrado, forma con ancho adelantado, contenido escalonado, pulso al
  cambiar de pista (puro y probado en `nowplaying_view.rs`); el temporizador sigue la frecuencia de
  la pantalla (60–120 Hz). La ventana va por encima de la barra de menús aunque esté oculta
  (`setFloatingPanel` reinicia el nivel: se fija después).
- Diseño «Vinyl / ambient» (`design/notch-player/03-vinyl-ambient.html`): tarjeta 430x216, disco con carátula «cover» y ranuras, anillo de progreso, halo desenfocado y luz teñida (color dominante); `nowplaying_view.rs` (geometría y muelles de la maqueta: k 150, c 14, impulso .8, escalonado 70 ms, altura +90 ms, cierre tras 400 ms) y `nowplaying_art.rs` (disco, halo y color a partir de píxeles) son puros y probados; `macos/nowplaying_art.rs` decodifica y prepara cada carátula una vez, en el hilo de lectura, y todo se dibuja en un solo `drawRect`.
- Entrada: `nowplaying_input.rs` (recorte «cover» de la carátula, qué texto abre la app, zona de clic de la barra y gesto de arrastre; un solo `seek` al soltar, en microsegundos con `perl … seek N`) y `Ctl::seek_to` (posición optimista que se mantiene 3 s hasta que el adaptador la confirme).
- Comprobación manual del framework empaquetado:
  `/usr/bin/perl <App>/Contents/Resources/mediaremote-adapter.pl <App>/Contents/Frameworks/MediaRemoteAdapter.framework get`
  (imprime `null` si no suena nada). El ecualizador es decorativo.
- Lógica pura y con pruebas en `crates/core/src/nowplaying.rs` (máquina de estados mostrar/ocultar/expandir, progreso),
  `nowplaying_feed.rs` (líneas JSON del adaptador, tiempo transcurrido, comandos, reinicio con
  espera y abandono) y `nowplaying_view.rs` (geometría,
  muelle, ecualizador, ritmo del temporizador). AppKit fino en `crates/macos/src/notch_player.rs`;
  proceso hijo, lectura y reinicios fuera del hilo principal en `crates/macos/src/nowplaying.rs`.
- Ejemplo para verlo sin música: `cargo run -p ferrocast-macos --example notch_player_demo -- --shots <dir>`.

## Pendiente

El estado final de funciones y la evidencia de pruebas se publicarán en README
al terminar la comprobación. Los permisos TCC de macOS deben concederse por el
usuario; ningún script intenta modificarlos ni desactivar las protecciones.
