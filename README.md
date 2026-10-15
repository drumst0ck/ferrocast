# Ferrocast

Launcher para macOS escrito en Rust, inspirado en Raycast. No es una réplica completa
ni usa marca o assets de Raycast. Investigación de funciones: [`docs/RESEARCH.md`](docs/RESEARCH.md).

## Uso rápido

```bash
scripts/bundle.sh          # compila y crea dist/Ferrocast.app (firma ad-hoc, solo local)
open dist/Ferrocast.app    # atajo por defecto: ⌥ Space
cargo test --workspace     # 59 tests
```

Requiere Rust 1.95 (fijado en `rust-toolchain.toml`; lo exige egui 0.36) y macOS en Apple Silicon.
Los datos viven en `~/Library/Application Support/Ferrocast` (JSON sin cifrar;
`FERROCAST_DATA_DIR` cambia la ruta).

## Estado

| Función | Estado |
|---|---|
| Root Search (apps, comandos, ranking con frecency, alias, favoritos) | Implementado |
| Calculadora (mates, %, unidades), colores, detección de URL | Implementado; sin divisas ni zonas horarias |
| Panel de acciones ⌘K y atajos | Implementado |
| Historial del portapapeles (texto, archivos, imágenes, pin) | Implementado; ignora marcas de gestores de contraseñas |
| Snippets con placeholders y auto-expansión | Implementado; necesita permisos |
| Quicklinks con argumentos | Implementado |
| Script Commands compatibles con Raycast | Implementado (full, compact, silent, inline) |
| Gestión de ventanas (~40 comandos) | Implementado; necesita Accesibilidad |
| Comandos de sistema, emoji, archivos (Spotlight), cambiar ventana, notas | Implementado |
| Icono en barra de menú, ajustes, hotkeys por comando | Implementado |
| Extensiones React, AI, sync, Search Menu Bar Items, divisas | No incluido |

## Qué está verificado y qué no

- Verificado: 59 tests (núcleo, bindings, estado de la UI con egui sin ventana) y arranque oculto del `.app`.
- **No verificado**: el aspecto de la ventana, foco, hotkeys globales, pegado, expansión de
  snippets y ventanas en una sesión real; no hubo permiso de captura de pantalla ni de Accesibilidad.
- Permisos (Accesibilidad / Input Monitoring) los concede el usuario. Al recompilar con firma
  ad-hoc macOS puede pedirlos de nuevo. Para distribuir hace falta firma Developer ID y notarización.
- Los emoji se muestran en monocromo (limitación de egui).
- Los scripts con `needsConfirmation` piden confirmación antes de ejecutarse.
