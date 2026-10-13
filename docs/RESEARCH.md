# Investigación: qué hace Raycast (macOS) y qué replica Ferrocast

Fuente principal: manual oficial (`https://manual.raycast.com/llms-full.txt`), API de
extensiones (`developers.raycast.com`) y repo `raycast/script-commands`.

## 1. Conceptos base

| Raycast | Descripción | Ferrocast |
|---|---|---|
| Launcher global (`⌥ Space` / `⌘ Space`) | Ventana flotante sin bordes, centrada, aparece con hotkey global y se oculta al perder foco | ✅ hotkey configurable, ventana flotante, auto‑hide |
| Root Search | Lista única: apps, comandos, quicklinks, snippets, scripts, archivos, calculadora, colores, URLs | ✅ |
| Ranking | 1) alias exacto 2) prefijo de alias 3) fuzzy título 4) subtítulo/keywords 5) frecency | ✅ mismo orden |
| Favoritos | Sección fija arriba con búsqueda vacía, reordenables | ✅ |
| Aliases & hotkeys por comando | Alias de texto y hotkey global por comando | ✅ |
| Action Panel `⌘K` | Acciones contextuales en secciones, buscable, primaria `↵`, secundaria `⌘↵` | ✅ |
| Argumentos de comando (máx 3) | Inputs inline en la barra de búsqueda | ✅ (quicklinks y scripts) |
| Fallback commands | Cuando no hay resultados: "Search Google…" etc. | ✅ |
| Compact mode | Solo la barra cuando la búsqueda está vacía | ✅ opción |
| Pop to root | Volver a Root Search tras inactividad | ✅ al reabrir |
| Navegación | `↑↓`, `⌘↑↓` secciones, `Esc` limpia/cierra, `⌫` vacío = atrás, `⌘,` ajustes | ✅ |

## 2. Funciones core

| Función | Raycast | Ferrocast (alcance) |
|---|---|---|
| Applications | Lanza apps (fuzzy), Show in Finder, Quit, Copy path | ✅ escaneo de `/Applications`, `/System/Applications`, `~/Applications`, iconos |
| Calculator | Math, %, unidades, divisas, fechas, zonas horarias, colores | ✅ math + funciones + `% of` / `% off` + unidades (longitud, masa, temperatura, datos, tiempo, volumen, área, velocidad, CSS px/rem) + colores. ⏳ divisas/zonas horarias |
| Color conversion | hex/rgb/hsl/… → copia en otros formatos | ✅ hex, rgb(a), hsl(a) → hex/rgb/hsl/NSColor/UIColor |
| Clipboard History | Texto, imágenes, archivos, links, colores; pin, borrar, pegar, ignorar password managers | ✅ texto + archivos + imágenes (PNG), pin, borrar, pegar, copiar, ignora tipos *concealed/transient* |
| Snippets | Biblioteca + auto‑expansión por keyword en cualquier app, placeholders | ✅ CRUD, búsqueda, pegar, auto‑expansión (CGEventTap) |
| Dynamic Placeholders | `{clipboard}`, `{date}`, `{time}`, `{datetime}`, `{day}`, `{uuid}`, `{cursor}`, `{argument}`, modificadores `uppercase` `lowercase` `trim` `percent-encode` `json-stringify`, `offset`, `format` | ✅ todos los listados (formato de fecha estilo Unicode/ICU básico) |
| Quicklinks | URL/archivo/deeplink con `{argument name="q"}` | ✅ CRUD, argumentos, abrir |
| Script Commands | Metadatos `@raycast.*`, modos `fullOutput` `compact` `silent` `inline`, argumentos, ANSI | ✅ mismo formato (compatible con scripts de Raycast) |
| Window Management | Mitades, cuartos, tercios, sextos, maximizar, centrar, restaurar, displays, gaps | ✅ vía Accessibility API |
| System Commands | Lock, sleep, restart, shutdown, logout, empty trash, appearance, volume, hide apps, quit all, eject | ✅ |
| File Search | Búsqueda indexada | ✅ vía Spotlight (`mdfind`) |
| Switch Windows | Lista de ventanas abiertas | ✅ (CGWindowList + activar app) |
| Search Menu Bar Items | Menús de la app frontal | ⏳ |
| Emoji & Symbols | Búsqueda por nombre, pegar | ✅ (crate `emojis`) |
| Notes | Notas markdown flotantes | ✅ notas en texto plano/markdown |
| Focus, Calendar, Translate, Screenshots (OCR), Auto Quit, Hyper Key | — | ⏳ fuera del MVP |
| AI (Quick AI, Chat, Commands, MCP…) | IA integrada | ⏳ futuro: endpoint OpenAI‑compatible / Ollama |
| Extensiones React | API TS/React renderizada nativa | ❌ (los Script Commands cubren la extensibilidad) |
| Cloud Sync, Teams, Enterprise | Servicios de cuenta | ❌ |
| Themes | Temas | ✅ claro/oscuro/sistema |
| Import/Export | Backup de datos | ✅ los datos son JSON legibles en `~/Library/Application Support/Ferrocast` |

Leyenda: ✅ implementado · ⏳ planificado · ❌ fuera de alcance.

## 3. Decisiones técnicas

- **UI**: `eframe/egui` (Rust puro, ventana sin bordes, transparente, always‑on‑top).
  Alternativas descartadas: Tauri (webview), gpui (dependencia git inestable), AppKit
  directo (mucho más código para el MVP).
- **Hotkeys globales**: `global-hotkey` (Carbon `RegisterEventHotKey`, no requiere permisos).
- **Menu bar**: `tray-icon`.
- **macOS nativo**: `objc2-*` para NSWorkspace/NSPasteboard/NSRunningApplication,
  `core-graphics` para CGEvent (pegar con `⌘V`, event tap de snippets),
  `accessibility-sys` para mover/redimensionar ventanas.
- **Lógica pura en `ferrocast-core`** (testeable sin GUI): fuzzy, ranking, frecency,
  calculadora, colores, placeholders, parser de scripts, modelos.
- **Persistencia**: JSON en `~/Library/Application Support/Ferrocast/`.
- **Permisos**: Accessibility (ventanas, pegar, snippets) — se solicita al usarlo.
