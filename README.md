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
| Conversión de archivos (imagen, audio, vídeo, documentos, hojas, subtítulos, carpetas→ZIP) con herramientas de macOS; ffmpeg, LibreOffice y cwebp opcionales | Implementado; 17 conversiones probadas con archivos reales |
| Herramientas de imagen: comprimir, redimensionar, recortar, quitar metadatos, PDF combinado, rotar, voltear, fondo, collage, ajustes, redactar caras/texto (Vision), anotar en Vista Previa | Implementado; probado con archivos reales (anotar y caras: sin prueba con foto real de una cara) |
| Acciones de archivo (copiar, copiar ruta, mover a…, Terminal/iTerm, AirDrop) y File Shelf | Implementado; AirDrop y Terminal no probados en sesión real |
| Rueda al arrastrar archivos (Shift: conversiones, Control: acciones) | Implementado; dibujo verificado por captura, el gesto real sin probar |
| Estantería flotante en ventana aparte anclada a un borde | No incluido |
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
