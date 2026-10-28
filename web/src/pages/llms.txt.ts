import { familyHubs } from '../lib/seo/convert';
import { SITE } from '../lib/seo/jsonld';
import { counts } from '../data/status';

export async function GET() {
  const { pairs, inputs, tools } = counts();
  const lines = [
    '# Ferrocast',
    '',
    '> Fast, open-source launcher for macOS (Rust, MIT). Search, calculate, clipboard, snippets, Ask AI (Ollama by default), Raycast-compatible Script Commands, ~40 window commands, screen recording with auto zoom, floating Markdown notes, and local file work: {pairs} conversions across {inputs} input formats and {tools} file tools. Apple Silicon only; no account.'.replace(
      '{pairs}',
      String(pairs),
    ).replace('{inputs}', String(inputs)).replace('{tools}', String(tools)),
    '',
    '## Key facts (quotable)',
    `- ${SITE}/facts/`,
    `- Source code and README: https://github.com/drumst0ck/ferrocast`,
    '',
    '## Primary pages',
    `- [Home](${SITE}/): overview, measured footprint, install (DMG and Homebrew)`,
    `- [Key facts and FAQ](${SITE}/facts/): plain facts for citations`,
    `- [Datos clave](${SITE}/es/facts/): los mismos datos en español`,
    `- [Raycast alternative](${SITE}/raycast-alternative/): what you keep, what is missing, and a sourced Ferrocast vs Raycast vs Alfred table`,
    `- [Historial del portapapeles que ignora contraseñas](${SITE}/es/historial-portapapeles-mac/): portapapeles de Ferrocast frente al de Spotlight en macOS Tahoe`,
    `- [Mac window management](${SITE}/mac-window-management/): the 40 window commands, compared with macOS tiling and Rectangle`,
    `- [Free file converter for Mac](${SITE}/free-file-converter-mac/): local conversion without uploads, compared with Permute and CloudConvert`,
    `- [Mac launcher with local AI](${SITE}/local-ai-launcher/): Ollama by default, LM Studio, your own keys or Claude Code, and which providers send text off the Mac`,
    `- [Free Screen Studio alternative](${SITE}/screen-studio-alternative/): screen recording with auto zoom, camera, on-device transcript, MP4 and GIF export`,
    `- [Grabar la pantalla con zoom automático](${SITE}/es/grabar-pantalla-mac-zoom-automatico/): lo mismo en español`,
    `- [Free Alfred alternative](${SITE}/alfred-alternative/): clipboard history and snippets without the Powerpack, with a sourced Alfred comparison`,
    `- [Ferrocast vs Raycast](${SITE}/ferrocast-vs-raycast/): Raycast Free vs Pro vs Ferrocast, feature by feature, with Raycast prices and sources`,
    `- [Run Raycast Script Commands without Raycast](${SITE}/raycast-script-commands/): which @raycast metadata Ferrocast reads, the four output modes and the limits`,
    `- [Compress a video to a target size](${SITE}/compress-video-to-size/): fit Discord (20 MiB) or Gmail (25 MB) limits on a Mac without uploading`,
    `- [Comprimir un vídeo a un tamaño](${SITE}/es/comprimir-video-a-un-tamano/): lo mismo en español`,
    `- [Convertir archivos en Mac](${SITE}/es/convertir/): conversiones más buscadas en español (HEIC a JPG, WebP a JPG, MOV a MP4, vídeo a GIF, PDF a Word)`,
    `- [Raycast vs Alfred vs Spotlight](${SITE}/es/raycast-vs-alfred-vs-spotlight/): comparativa en español con Spotlight de macOS Tahoe, con fuentes oficiales`,
    `- [Alternativa a Raycast](${SITE}/es/alternativa-a-raycast/): la misma comparativa en español, con qué conservas y qué falta`,
    `- [Convert and file tools](${SITE}/files/): interactive explorer of every conversion pair`,
    `- [File tools index](${SITE}/tools/): all local file tools`,
    `- [Documentation](${SITE}/docs/): install, launcher features, file tools reference`,
    `- [Changelog](${SITE}/changelog/): release notes`,
    '',
    '## Documentation highlights',
    `- [Install](${SITE}/docs/getting-started/install/)`,
    `- [Conversion reference](${SITE}/docs/files/conversions/)`,
    `- [Optional tools (ffmpeg, LibreOffice, …)](${SITE}/docs/files/optional-tools/)`,
    `- [Organize PDF pages](${SITE}/tools/organize-pdf/): PDF organizer for Mac. Reorder, rotate, duplicate and remove pages offline`,
    `- [Migrating from Raycast](${SITE}/docs/getting-started/migrating-from-raycast/)`,
    `- [Data and privacy](${SITE}/docs/reference/data-and-privacy/)`,
    `- [FAQ in docs](${SITE}/docs/reference/faq/)`,
    '',
    '## Conversions by family',
    ...familyHubs().map((f) => `- [${f.name}](${SITE}/convert/${f.id}/)`),
    '',
  ];
  return new Response(lines.join('\n'), { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
