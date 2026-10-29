<p align="center"><img src="web/public/icon.png" width="96" alt="Icono de Ferrocast"></p>

# Ferrocast

**El lanzador rápido de código abierto para Mac que también convierte archivos en local.**

Lanzador de código abierto para macOS, escrito en Rust. Busca, calcula, pega y pregunta a una IA desde el teclado; arrastra cualquier archivo, pulsa una tecla y conviértelo, comprímelo, recórtalo, córtalo u oculta partes en tu Mac. Las conversiones y herramientas de archivo se ejecutan en tu Mac y no suben tus archivos.

[![CI](https://github.com/drumst0ck/ferrocast/actions/workflows/ci.yml/badge.svg)](https://github.com/drumst0ck/ferrocast/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[Website](https://ferrocast.app) · [Docs](https://ferrocast.app/docs/) · [Changelog](https://ferrocast.app/changelog/) · [English](README.md)

<p align="center"><img src="web/public/img/home.jpg" alt="Lanzador de Ferrocast" width="46%"> <img src="web/public/img/files-wheel-convert.jpg" alt="Rueda de conversión de Ferrocast" width="46%"></p>

## Datos clave

| | |
| --- | --- |
| **Licencia** | [MIT](LICENSE) |
| **Plataforma** | Solo Mac con Apple Silicon |
| **Tamaño** | Unos 8,5 MB en disco, un proceso ([método de medición](https://ferrocast.app/es/#bench)) |
| **Cuenta** | Ninguna; sin plan de pago |
| **Archivos** | 275 pares entre 46 formatos de entrada, 26 herramientas y 7 acciones — todo en local |
| **Lanzador** | Búsqueda, calculadora, portapapeles, snippets, quicklinks, ~40 comandos de ventanas, Script Commands compatibles con Raycast |
| **Ask AI** | [Ollama](https://ollama.com/) en tu Mac por defecto; proveedores en la nube opcionales |
| **Privacidad** | Las conversiones se quedan en disco; consulta [datos y privacidad](https://ferrocast.app/es/docs/reference/data-and-privacy/) para uso de red (divisas, IA opcional) |
| **Instalación** | [DMG](https://github.com/drumst0ck/ferrocast/releases/latest/download/Ferrocast.dmg) o Homebrew (abajo); la primera apertura del DMG puede pedir **Abrir igualmente** en Ajustes del Sistema |
| **Más** | [Datos clave y FAQ](https://ferrocast.app/facts/) · [llms.txt](https://ferrocast.app/llms.txt) para asistentes de IA |

## Instalar

**[Descarga el último DMG](https://github.com/drumst0ck/ferrocast/releases/latest/download/Ferrocast.dmg)**, arrastra Ferrocast a Aplicaciones y ábrela. Lleva firma ad-hoc, así que macOS pide «Abrir igualmente» la primera vez. Las copias descargadas se actualizan solas: Ajustes → General → Updates. Todas las versiones están en la [página de Releases](https://github.com/drumst0ck/ferrocast/releases).

**Alternativa: Homebrew.**

```bash
brew tap drumst0ck/ferrocast
brew install drumst0ck/ferrocast/ferrocast  # compila en tu Mac (necesita Rust, lo instala brew)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # concede Accesibilidad cuando lo pida
brew upgrade ferrocast                      # para actualizar
```

**Por qué una fórmula y no un cask.** El cask oficial de Homebrew exige apps firmadas y notarizadas (cuenta de Apple Developer de pago). Al compilar desde el código fuente en tu propia máquina, la app no lleva marca de cuarentena y Gatekeeper no interviene. No hace falta cuenta de pago y no se salta ninguna protección. La primera instalación tarda unos minutos porque compila, y la firma es ad-hoc, de uso local.

Ejecuta antes `brew tap`: Homebrew se niega a cargar fórmulas de un tap que no has añadido. La fórmula vive en [drumst0ck/homebrew-ferrocast](https://github.com/drumst0ck/homebrew-ferrocast) y sigue cada versión estable de forma automática. Solo Apple Silicon. Más en la [guía de instalación](https://ferrocast.app/es/docs/getting-started/install/).

## Qué hace

- **Buscar** apps, comandos y archivos con frecency, alias, favoritos y un atajo por comando.
- **Calcular** en la caja de búsqueda: operaciones, porcentajes, unidades, divisas (tasas del BCE) y husos horarios.
- **Historial del portapapeles** de texto, archivos e imágenes; ignora las entradas de gestores de contraseñas.
- **Snippets** con placeholders y auto-expansión, y **quicklinks** con argumentos.
- **Script Commands** compatibles con los metadatos de Raycast (full, compact, silent, inline).
- **Gestión de ventanas** con unos 40 diseños, más Kill Process, temporizadores, notas, emoji y Quick Look.
- **Notas**: notas flotantes en Markdown con imágenes (pegar, arrastrar o Insert Image…; PNG, JPEG, GIF, HEIC y WebP de hasta 20 MB), menú `/`, notas fijadas, exportación, enlaces `ferrocast://`, comandos de IA y dictado. Las notas de una bóveda de Obsidian no se editan en esa ventana.
- **Idiomas de la interfaz**: inglés, español, alemán, francés, portugués de Brasil, japonés y chino simplificado. Sigue el orden de idiomas de macOS, o Ajustes → General → Idioma. Se guarda en cada Mac y no se sincroniza. Lo que no está traducido vuelve al inglés. Los nombres que escribes y la interfaz de las extensiones de Raycast no se traducen.
- **Ask AI** con Ollama, cualquier servidor compatible con OpenAI, la API de Anthropic, Claude Code o Sign in with ChatGPT.

## Herramientas de archivo

- Arrastra un archivo y pulsa <kbd>⇧</kbd> para conversiones, <kbd>⌘</kbd> para herramientas o <kbd>⌃</kbd> para acciones.
- **275 conversiones** entre 46 formatos de entrada (imágenes, audio, vídeo, documentos, hojas de cálculo, PDF, presentaciones, subtítulos, archivos comprimidos).
- **26 herramientas**: comprimir, recortar, cortar, dividir, unir, ocultar zonas, anotar, editar metadatos, organizar páginas de PDF y más.
- **7 acciones**: AirDrop, estantería, copiar, mover, copiar ruta, Terminal y Zip.
- **Rueda de traducción**: arrastra texto seleccionado, pulsa <kbd>⌘</kbd> y suelta sobre un idioma. La traducción sustituye la selección en su sitio donde la app lo permite y, si no, se abre en una ventana. Usa tu proveedor de Ask AI (Ollama se queda en local; también sirven las suscripciones de ChatGPT y Claude).
- Todo en local. Primero las herramientas de macOS; ffmpeg, LibreOffice, cwebp, unrar y Ghostscript son opcionales. El resultado queda junto al original y nunca lo sobrescribe.

Explora todos los pares en <https://ferrocast.app/files/>.

## Capturas de pantalla

<p align="center">
  <img src="web/public/img/search.jpg" alt="Búsqueda" width="31%">
  <img src="web/public/img/clipboard.jpg" alt="Historial del portapapeles" width="31%">
  <img src="web/public/img/ai.jpg" alt="Ask AI" width="31%">
</p>
<p align="center">
  <img src="web/public/img/record-editor.jpg" alt="Editor de grabación de pantalla" width="48%">
  <img src="web/public/img/notes.jpg" alt="Notas flotantes" width="48%">
</p>

## Raycast y Alfred

Ferrocast es **independiente** y no está afiliado a [Raycast](https://www.raycast.com/) ni a [Alfred](https://www.alfredapp.com/). Esta tabla solo recoge lo que documenta este repositorio.

| Tema | Ferrocast | Notas para quienes vienen de otros lanzadores |
| --- | --- | --- |
| **Precio y código fuente** | MIT, código abierto, sin cuenta | Consulta la web de cada producto para el precio y la licencia |
| **Script Commands** | Metadatos compatibles con Raycast (full, compact, silent, inline) | Los Script Commands existentes funcionan tal cual ([guía de migración](https://ferrocast.app/es/docs/getting-started/migrating-from-raycast/)) |
| **Snippets** | Importación desde una exportación JSON de Raycast | Los quicklinks se recrean a mano |
| **Conversión y herramientas de archivo** | 275 pares de conversión en local y 26 herramientas de archivo integradas | No está documentado en este repositorio para Raycast ni para Alfred |
| **Extensiones y sincronización** | Sin tienda de extensiones propia. Las extensiones que ya instalaste con Raycast pueden ejecutarse (parcial, desactivado por defecto, Node.js 22+). Sincronización opcional por iCloud Drive, desactivada por defecto | Aún no están soportados: comandos de la barra de menús, OAuth, funciones de IA de las extensiones, selector de archivos y accesorio de enlace. Tampoco se incluye: Search Menu Bar Items ni contexto de IA desde la selección o la pantalla ([guía de migración](https://ferrocast.app/es/docs/getting-started/migrating-from-raycast/), [extensiones](https://ferrocast.app/es/docs/launcher/raycast-extensions/), [sincronización](https://ferrocast.app/es/docs/launcher/sync/)) |

## Qué falta todavía

- Una **tienda de extensiones propia**. Ferrocast no contacta con la Store de Raycast y no está afiliado a Raycast.
- **Compatibilidad completa con las extensiones de Raycast.** En una prueba, funcionaron 34 de 40 comandos de vista de 20 extensiones populares. Aún no están soportados los comandos de la barra de menús, el inicio de sesión OAuth, las funciones de IA, el selector de archivos y el accesorio de enlace.
- **Search Menu Bar Items** y **contexto de IA** desde la selección o la pantalla.

## Ask AI y privacidad

De serie, Ask AI habla con Ollama en tu propio Mac. Puedes apuntarlo a LM Studio, OpenAI o la API de Anthropic (las claves van al Keychain), o usar Claude Code con tu plan de Claude. Sign in with ChatGPT usa el mismo flujo que la CLI Codex de OpenAI; OpenAI no ha autorizado el uso por terceros, así que la API con clave es la vía correcta.

Los datos viven en `~/Library/Application Support/Ferrocast` como JSON sin cifrar (`FERROCAST_DATA_DIR` cambia la ruta). Con la sincronización con iCloud activada (Ajustes → Advanced, desactivada por defecto), los ajustes, snippets, quicklinks, notas, el historial del portapapeles, los chats de Ask AI y los temas propios también se copian a iCloud Drive. Las claves del Llavero se quedan en cada Mac.

## Compilar desde el código

```bash
scripts/bundle.sh          # compila dist/Ferrocast.app (firma ad-hoc, solo uso local)
scripts/dmg.sh             # dist/Ferrocast-<versión>.dmg
open dist/Ferrocast.app    # atajo por defecto: ⌥ Space
cargo test --workspace
```

Rust 1.95 está fijado en `rust-toolchain.toml` (lo exige egui 0.36). El DMG sin notarizar lo bloquea Gatekeeper en otros Macs; la vía soportada es Homebrew. Consulta [CONTRIBUTING.md](CONTRIBUTING.md).

## Documentación

[ferrocast.app/es/docs](https://ferrocast.app/es/docs/) · [Changelog](https://ferrocast.app/changelog/) ([RSS](https://ferrocast.app/changelog/rss.xml)) · [Investigación de funciones](docs/RESEARCH.md)

## Licencia

MIT, ver [LICENSE](LICENSE).
