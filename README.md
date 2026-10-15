# Ferrocast

Launcher para macOS escrito en Rust, inspirado en Raycast. No es una réplica completa
ni usa marca o assets de Raycast. Investigación de funciones: [`docs/RESEARCH.md`](docs/RESEARCH.md).

## Uso rápido

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
| Icono en barra de menú, ajustes, hotkeys por comando | Implementado |
| Emoji en color (renderizados con AppKit) | Implementado |
| Ask AI con streaming y conversación (servidor compatible con OpenAI: Ollama por defecto, LM Studio, OpenAI…; clave en el Keychain) | Implementado; probado contra un servidor simulado, no contra un modelo real |
| Arranque al iniciar sesión (LaunchAgent) | Implementado; se aplica en el siguiente inicio de sesión |
| Bienvenida y permisos, atajos, Acerca de | Implementado |
| Exportar copia de seguridad (sin historial del portapapeles), importar snippets (JSON de Raycast) | Implementado |
| Instancia única, log de errores (`Reveal Log File`) | Implementado |
| Icono, entitlements, DMG, script de notarización | Implementado; la notarización no se ha ejecutado (requiere cuenta de desarrollador) |
| Extensiones React, sync, Search Menu Bar Items, contexto de IA (selección/pantalla) | No incluido |

## Distribución

La firma por defecto es ad-hoc (solo uso local). Para repartirlo: identidad *Developer ID*, `FERROCAST_SIGN_IDENTITY`
y `scripts/notarize.sh` (ver `CONTRIBUTING.md`). Sin notarizar, Gatekeeper avisará en otros Macs.

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
