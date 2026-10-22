<p align="center"><img src="web/public/icon.png" width="96" alt="Icono de Ferrocast"></p>

# Ferrocast

Lanzador de código abierto para macOS, escrito en Rust. Busca, calcula, pega y pregunta a una IA desde el teclado; arrastra cualquier archivo, pulsa una tecla y conviértelo, comprímelo, recórtalo, córtalo u oculta partes en tu Mac. No se sube nada.

[![CI](https://github.com/drumst0ck/ferrocast/actions/workflows/ci.yml/badge.svg)](https://github.com/drumst0ck/ferrocast/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[Website](https://ferrocast.app) · [Docs](https://ferrocast.app/docs/) · [Changelog](https://ferrocast.app/changelog/) · [English](README.md)

<p align="center"><img src="web/public/img/home.jpg" alt="Lanzador de Ferrocast" width="46%"> <img src="web/public/img/files-wheel-convert.jpg" alt="Rueda de conversión de Ferrocast" width="46%"></p>

## Instalar

```bash
brew tap drumst0ck/ferrocast https://github.com/drumst0ck/ferrocast
brew install ferrocast                      # compila en tu Mac (necesita Rust, lo instala brew)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # concede Accesibilidad cuando lo pida
brew upgrade ferrocast                      # para actualizar
```

¿Prefieres una descarga? Cada versión estable está en la [página de Releases](https://github.com/drumst0ck/ferrocast/releases) como DMG (firma ad-hoc: macOS pide «Abrir igualmente» la primera vez). Las copias descargadas se actualizan solas: Ajustes → General → Updates.

**Por qué una fórmula y no un cask.** El cask oficial de Homebrew exige apps firmadas y notarizadas (cuenta de Apple Developer de pago). Al compilar desde el código fuente en tu propia máquina, la app no lleva marca de cuarentena y Gatekeeper no interviene. No hace falta cuenta de pago y no se salta ninguna protección. La primera instalación tarda unos minutos porque compila, y la firma es ad-hoc, de uso local.

Usa siempre el `brew tap` con la URL: la forma corta `brew install drumst0ck/ferrocast/ferrocast` intenta clonar `homebrew-ferrocast`, que no existe. Solo Apple Silicon. Más en la [guía de instalación](https://ferrocast.app/es/docs/getting-started/install/).

## Qué hace

- **Buscar** apps, comandos y archivos con frecency, alias, favoritos y un atajo por comando.
- **Calcular** en la caja de búsqueda: operaciones, porcentajes, unidades, divisas (tasas del BCE) y husos horarios.
- **Historial del portapapeles** de texto, archivos e imágenes; ignora las entradas de gestores de contraseñas.
- **Snippets** con placeholders y auto-expansión, y **quicklinks** con argumentos.
- **Script Commands** compatibles con los metadatos de Raycast (full, compact, silent, inline).
- **Gestión de ventanas** con unos 40 diseños, más Kill Process, temporizadores, notas, emoji y Quick Look.
- **Ask AI** con Ollama, cualquier servidor compatible con OpenAI, la API de Anthropic, Claude Code o Sign in with ChatGPT.

No incluido: extensiones React, sincronización, Search Menu Bar Items y contexto de IA desde la selección o la pantalla. Ferrocast es independiente y no está afiliado a Raycast.

## Herramientas de archivo

- Arrastra un archivo y pulsa <kbd>⇧</kbd> para conversiones, <kbd>⌘</kbd> para herramientas o <kbd>⌃</kbd> para acciones.
- **275 conversiones** entre 46 formatos de entrada (imágenes, audio, vídeo, documentos, hojas de cálculo, PDF, presentaciones, subtítulos, archivos comprimidos).
- **26 herramientas**: comprimir, recortar, cortar, dividir, unir, ocultar zonas, anotar, editar metadatos, organizar páginas de PDF y más.
- **7 acciones**: AirDrop, estantería, copiar, mover, copiar ruta, Terminal y Zip.
- **Rueda de traducción**: arrastra texto seleccionado, pulsa <kbd>⌘</kbd> y suelta sobre un idioma. La traducción sustituye la selección en su sitio donde la app lo permite y, si no, se abre en una ventana. Usa tu proveedor de Ask AI (Ollama se queda en local; también sirven las suscripciones de ChatGPT y Claude).
- Todo en local. Primero las herramientas de macOS; ffmpeg, LibreOffice, cwebp, unrar y Ghostscript son opcionales. El resultado queda junto al original y nunca lo sobrescribe.

Explora todos los pares en <https://ferrocast.app/es/files/>.

## Ask AI y privacidad

De serie, Ask AI habla con Ollama en tu propio Mac. Puedes apuntarlo a LM Studio, OpenAI o la API de Anthropic (las claves van al Keychain), o usar Claude Code con tu plan de Claude. Sign in with ChatGPT usa el mismo flujo que la CLI Codex de OpenAI; OpenAI no ha autorizado el uso por terceros, así que la API con clave es la vía correcta.

Los datos viven en `~/Library/Application Support/Ferrocast` como JSON sin cifrar (`FERROCAST_DATA_DIR` cambia la ruta).

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
