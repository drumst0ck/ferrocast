# Ferrocast

Launcher para macOS escrito en Rust, inspirado en Raycast. No es una réplica completa
ni usa marca o assets de Raycast. Investigación de funciones: [`docs/RESEARCH.md`](docs/RESEARCH.md).

## Instalar con Homebrew (sin cuenta de desarrollador)

```bash
brew tap drumst0ck/ferrocast https://github.com/drumst0ck/ferrocast
brew install ferrocast                      # compila en tu Mac (necesita Rust, lo instala brew)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # concede Accesibilidad cuando lo pida
brew upgrade ferrocast                      # para actualizar
```

**Por qué una fórmula y no un cask.** El cask oficial de Homebrew exige apps firmadas y notarizadas
(Apple Developer, de pago) y desactiva las que no pasan Gatekeeper desde septiembre de 2026. Al compilar
desde el código fuente en tu propia máquina, la app **no lleva marca de cuarentena** y Gatekeeper no
interviene: no hace falta cuenta de pago ni saltarse ninguna protección. A cambio, la primera
instalación tarda unos minutos (compila) y la firma es ad-hoc, de uso local.

**Verificado.** La instalación desde el tap público (`brew tap` con URL + `brew install ferrocast`) se probó de extremo a extremo:
compila en ~2,5 min, la app no lleva cuarentena, `codesign --verify --strict` pasa y arranca vía LaunchServices.
Usa siempre el `brew tap` con la URL: la forma corta `brew install drumst0ck/ferrocast/ferrocast` intenta clonar
`homebrew-ferrocast`, que no existe, y falla.

**Permisos al actualizar.** La firma ad-hoc se ancla al identificador `dev.ferrocast.launcher` en lugar
de al hash del binario, de modo que macOS debería conservar el permiso de Accesibilidad entre versiones.
Esto no está verificado en una sesión real; si tras `brew upgrade` te lo vuelve a pedir, concédelo de nuevo.

## Uso rápido (desde el código)

```bash
scripts/bundle.sh          # compila y crea dist/Ferrocast.app con icono (firma ad-hoc, solo local)
scripts/dmg.sh             # dist/Ferrocast-<versión>.dmg
open dist/Ferrocast.app    # atajo por defecto: ⌥ Space; la primera vez muestra Welcome & Permissions
cargo test --workspace     # ~70 tests
scripts/shot.sh out.png FERROCAST_QUERY="100 usd in eur"   # render the UI to a PNG (no screen permission needed)
```

Requiere Rust 1.95 (fijado en `rust-toolchain.toml`; lo exige egui 0.36) y macOS en Apple Silicon.
Los datos viven en `~/Library/Application Support/Ferrocast` (JSON sin cifrar;
`FERROCAST_DATA_DIR` cambia la ruta).

## Estado

| Función | Estado |
|---|---|
| Root Search (apps, comandos, ranking con frecency, alias, favoritos) | Implementado |
| Calculadora (mates, %, unidades), colores, detección de URL | Implementado |
| Divisas (tasas BCE vía frankfurter.dev, caché 12 h) y husos horarios (`time in tokyo`, `5pm ldn in sf`) | Implementado; las divisas requieren red |
| Panel de acciones ⌘K y atajos | Implementado |
| Historial del portapapeles (texto, archivos, imágenes, pin) | Implementado; ignora marcas de gestores de contraseñas |
| Snippets con placeholders y auto-expansión | Implementado; necesita permisos |
| Quicklinks con argumentos | Implementado |
| Script Commands compatibles con Raycast | Implementado (full, compact, silent, inline) |
| Gestión de ventanas (~40 comandos) | Implementado; necesita Accesibilidad |
| Comandos de sistema, emoji, archivos (Spotlight), cambiar ventana, notas | Implementado |
| Kill Process (procesos por CPU y memoria, SIGTERM o SIGKILL con ⌘↵) | Implementado |
| Temporizadores (`timer 10m`, `pomodoro`; aviso y sonido al terminar; vista Timers con cancelación) | Implementado |
| Toggle Wi-Fi y Toggle Keep Awake (caffeinate) como comandos de sistema | Implementado; no probados en sesión real |
| Quick Look de archivos (⌘Y) y «Check for Updates» vía Homebrew | Implementado; no probados en sesión real |
| Calculator History (últimos 100 cálculos, borrar entrada o todo), emoji recientes, «Look up in Dictionary» como fallback | Implementado |
| Icono en barra de menú, ajustes, hotkeys por comando | Implementado |
| Emoji en color (renderizados con AppKit) | Implementado |
| Ask AI con streaming y conversación. Cuatro proveedores: servidor compatible con OpenAI (Ollama por defecto, LM Studio, OpenAI…), **Sign in with ChatGPT** (mismo flujo OAuth y backend que Codex CLI; usa tu plan de ChatGPT; zona gris de términos, avisado en la app), API de Anthropic (clave propia) y **Claude Code** (la CLI oficial, con tu suscripción a Claude). Claves en el Keychain | Implementado; OpenAI probado contra un servidor simulado, Claude Code probado en real (`cargo test claude_code_live -- --ignored`), Anthropic API sin probar en real |
| Arranque al iniciar sesión (LaunchAgent) | Implementado; se aplica en el siguiente inicio de sesión |
| Bienvenida y permisos, atajos, Acerca de | Implementado |
| Exportar copia de seguridad (sin historial del portapapeles), importar snippets (JSON de Raycast) | Implementado |
| Instancia única, log de errores (`Reveal Log File`) | Implementado |
| Icono, entitlements, DMG, fórmula de Homebrew | Implementado; el script de notarización es opcional y no se ha ejecutado |
| Conversión de archivos: los 46 formatos de entrada y los 275 pares de la tabla de Tangerine (imagen, audio, vídeo, documentos, hojas, PDF, presentaciones, EPUB, subtítulos, archivos comprimidos); herramientas de macOS primero, ffmpeg, LibreOffice y cwebp opcionales | Implementado; los 275 pares se ejecutan sobre archivos reales generados y se lee el resultado (texto dentro del documento, ancho de imagen, duración de audio/vídeo). No demuestra fidelidad de maquetación; RAR solo con un fichero sintético, porque el `unrar` de este Mac es x86; `.pages` escrito en formato iWork '09 y abierto solo con LibreOffice, no con Pages |
| Herramientas de imagen: comprimir, redimensionar, recortar, quitar metadatos, PDF combinado, rotar, voltear, fondo (color o copia desenfocada, margen, esquinas, sombra: `frame blur 40`), collage (cuadrícula, fila, columna, destacado: `collage row 1600 gap 16`), ajustes (`edit brightness 20 grain 30 dehaze 50`), redactar caras/texto (Vision) o áreas escritas con color o desenfoque (`redact 10,10 200x100 blur`), anotar (lienzo propio; con varias imágenes, Vista Previa) | Implementado como órdenes escritas, probado con archivos reales; la vista previa en vivo y el lienzo están en las filas siguientes (⇧⌘A, ⇧⌘E, ⇧⌘K); otra imagen como fondo con `frame image /ruta/fondo.jpg`; caras: sin prueba con foto real de una cara |
| Acciones de archivo (copiar, copiar ruta, mover a…, Terminal/iTerm, AirDrop) y File Shelf | Implementado; AirDrop y Terminal no probados en sesión real |
| Rueda al arrastrar archivos (Shift: conversiones, Control: acciones) | Implementado; dibujo verificado por captura, el gesto real sin probar |
| Estantería flotante (panel nativo siempre encima, anclado a un borde, arrastrar dentro y fuera) | Implementado; no probado en sesión real |
| Rueda de herramientas (Command), compresión por tamaño, recorte exacto, metadatos EXIF/GPS, recortar/dividir/unir | Implementado; probado con archivos reales |
| Herramientas de audio y vídeo (ffmpeg): silenciar, velocidad, fotograma, recortar encuadre, tapar con caja o desenfoque, unir clips (silencio generado en los mudos), normalizar volumen, visualizador (onda o imagen fija), quitar silencios de los extremos, mono/estéreo, pitido; organizar páginas de PDF (`pages 3,1,2r`) | Implementado como órdenes escritas; probado con archivos reales y revisado por un verificador independiente sin bloqueantes. Normalizar es de una pasada y el aviso muestra la sonoridad medida antes y después (LUFS), sin forma de onda; WMV falla en silenciar/recortar encuadre; sin editor visual ni forma de onda |
| Rueda de formatos desde un archivo con el teclado (⌥↵ en un resultado; Shift/Command/Control cambian de rueda) | Implementado; sin prueba de teclas en sesión real. Intro sobre un archivo sigue abriéndolo; Intro sobre la selección de Finder, no incluido |
| Organizador visual de PDF (⇧⌘O sobre un PDF: miniaturas, girar, duplicar, quitar y reordenar páginas, guarda una copia) | Implementado; dibujo verificado por captura, teclas sin probar en sesión real; sin arrastrar para reordenar |
| Lienzo de anotación propio (⇧⌘A sobre una imagen: flecha, línea, caja, elipse, resaltado, lápiz, etiquetas numeradas y lupa; guarda una copia PNG a tamaño completo) | Implementado; dibujo verificado por captura y guardado con archivo real; ratón y teclas sin probar en sesión real; con texto libre (fuente Arial del sistema); con bocadillos con puntero e imágenes superpuestas (selector de archivos, sin probar en sesión real); «Anotar» del menú de herramientas abre el lienzo con una imagen y Vista Previa con varias |
| Editor de fotos con vista previa en vivo (⇧⌘E: nueve deslizadores, copia a tamaño completo con ↵) | Implementado; dibujo verificado por captura, deslizadores sin probar en sesión real; la vista previa usa una copia reducida |
| Editor de recorte (⇧⌘K: rectángulo arrastrable con asas, proporciones fijas, ancho y alto exactos; copia con ↵) | Implementado; dibujo verificado por captura y recorte con archivo real; arrastre sin probar en sesión real |
| Editor de redacción (⇧⌘R: arrastrar para añadir áreas, moverlas y cambiar su tamaño, color sólido o desenfoque; copia con ↵) | Implementado; dibujo verificado por captura, arrastre sin probar en sesión real; la vista previa solo marca las áreas |
| Editor de fondo (⇧⌘B: margen, esquinas y sombra con deslizadores, fondo de color o copia desenfocada; copia con ↵) | Implementado; dibujo verificado por captura, deslizadores sin probar en sesión real; otra imagen como fondo solo con `frame image` |
| Editor de collage (con 2 o más imágenes: cuadrícula, fila, columna o destacado; ancho, separación, esquinas y fondo; copia con ↵) | Implementado; dibujo verificado por captura, deslizadores sin probar en sesión real; sin reordenar imágenes arrastrando |
| Visor de recorte de vídeo (⇧⌘V: fotograma en el cursor, marcadores de inicio y fin, paso por fotograma o segundo, corte exacto recodificado con ↵; requiere ffmpeg) | Implementado; dibujo verificado por captura con un fotograma real, teclas y barra sin probar en sesión real; latencia de la vista previa sin medir |
| Ventana de recorte de encuadre de vídeo (⇧⌘J: fotograma real, rectángulo arrastrable, proporciones, ancho y alto exactos; copia con ↵; requiere ffmpeg) | Implementado; dibujo verificado por captura, arrastre sin probar en sesión real; el fotograma de referencia es fijo (10 % de la duración) |
| Ventanas con vista previa de velocidad, fotogramas y redacción de vídeo | No incluido: solo órdenes escritas |
| Extensiones React, sync, Search Menu Bar Items, contexto de IA (selección/pantalla) | No incluido |

## Distribución

La vía soportada es Homebrew compilando desde fuente (arriba). El DMG de `scripts/dmg.sh` **sin notarizar**
lo bloqueará Gatekeeper en otros Macs que lo descarguen; solo sirve para uso propio. La notarización
(`scripts/notarize.sh`) es opcional y requiere cuenta de desarrollador de pago.

## Qué está verificado y qué no

- Verificado: tests (núcleo, bindings, estado de la UI), arranque del `.app`, y el aspecto real de la
  ventana mediante capturas internas (`scripts/shot.sh`): búsqueda, calculadora, divisas, husos,
  portapapeles con detalle, panel ⌘K, emoji y ajustes.
- **No verificado en sesión real**: foco al abrir/cerrar, hotkeys globales, pegado, expansión de
  snippets y gestión de ventanas (requieren Accesibilidad, que concede el usuario).
- Permisos (Accesibilidad / Input Monitoring) los concede el usuario. Al recompilar con firma
  ad-hoc macOS puede pedirlos de nuevo. Para distribuir hace falta firma Developer ID y notarización.
- Los scripts con `needsConfirmation` piden confirmación antes de ejecutarse.
- Los argumentos de Script Commands (incluidos los de tipo `password`) se pasan por línea de comandos,
  como en Raycast, y pueden verse con `ps` mientras el script corre. No pongas secretos críticos ahí.
