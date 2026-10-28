/*
 * Quotable key facts for /facts/ and /es/facts/. Counts come from data/status.ts; the minimum macOS
 * version is LSMinimumSystemVersion in scripts/bundle.sh (13.0); keep them in step.
 */
import { counts } from '../../data/status';

export const MIN_MACOS = { version: '13', name: 'Ventura' };

export interface FactsCopy {
  lang: 'en' | 'es';
  path: string;
  title: string;
  description: string;
  crumbs: [string, string][];
  h1: string;
  lead: string;
  factsH: string;
  facts: string[];
  linksH: string;
  links: [string, string][];
  faqH: string;
}

export function factsCopy(lang: 'en' | 'es'): FactsCopy {
  const c = counts();
  if (lang === 'es') {
    return {
      lang,
      path: '/es/facts/',
      title: 'Ferrocast: datos clave y preguntas frecuentes',
      description:
        'Datos claros sobre Ferrocast, el lanzador de código abierto para Mac con Apple Silicon: conversión local de archivos, instalación, privacidad y lo que falta.',
      crumbs: [['Inicio', '/es/'], ['Datos clave', '/es/facts/']],
      h1: 'Datos clave',
      lead: 'Datos breves y citables sobre Ferrocast, para personas y asistentes. Fuentes: este repositorio, [la documentación](/es/docs/) y [GitHub](https://github.com/drumst0ck/ferrocast).',
      factsH: 'De un vistazo',
      facts: [
        'Ferrocast es un lanzador de teclado rápido y de código abierto para macOS, escrito en Rust (licencia MIT).',
        `Solo Mac con Apple Silicon; no funciona en Intel. La app declara macOS ${MIN_MACOS.version} (${MIN_MACOS.name}) o posterior como mínimo; la grabación de pantalla necesita macOS 14 o posterior.`,
        'Unos 8,5 MB en disco y un solo proceso en reposo (medido en el Mac del autor; ver la nota de método en la página de inicio).',
        `${c.pairs} conversiones de archivos en local entre ${c.inputs} formatos de entrada, más ${c.tools} herramientas y ${c.actions} acciones de archivo desde una rueda de arrastrar y soltar.`,
        'Unos 40 comandos de ventanas, historial del portapapeles, snippets, quicklinks, calculadora y notas Markdown flotantes.',
        'Ask AI usa Ollama en tu Mac por defecto; puedes apuntarlo a otros proveedores. Las claves de API se guardan en el Llavero.',
        'Script Commands compatibles con Raycast; los snippets se importan desde una exportación JSON de Raycast.',
        'Las conversiones y herramientas de archivo no suben tus archivos; el resultado se guarda junto al original.',
        'Sin cuenta y sin plan de suscripción.',
        'Se instala con el DMG (la primera vez puede pedir «Abrir igualmente» en Ajustes del Sistema) o con Homebrew (se compila en tu Mac).',
        'Herramientas opcionales, si están instaladas: ffmpeg, LibreOffice, Ghostscript, cwebp y unrar.',
        'Aún no incluye: tienda de extensiones, sincronización entre Macs, extensiones React, Search Menu Bar Items ni contexto de IA desde la selección o la pantalla.',
        'Ferrocast es independiente y no está afiliado a Raycast ni a Alfred.',
      ],
      linksH: 'Más información',
      links: [
        ['Inicio e instalación', '/es/'],
        ['Explorar las conversiones', '/es/files/'],
        ['Herramientas de archivo', '/es/tools/'],
        ['Convertir archivos en Mac', '/es/convertir/'],
        ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
        ['Raycast vs Alfred vs Spotlight', '/es/raycast-vs-alfred-vs-spotlight/'],
        ['Migrar desde Raycast', '/es/docs/getting-started/migrating-from-raycast/'],
        ['Datos y privacidad', '/es/docs/reference/data-and-privacy/'],
        ['Cambios', '/changelog/'],
      ],
      faqH: 'Preguntas frecuentes',
    };
  }
  return {
    lang,
    path: '/facts/',
    title: 'Ferrocast key facts and FAQ',
    description:
      'Plain facts about Ferrocast: the open-source Apple Silicon Mac launcher, local file conversion, install, privacy, Raycast migration, and gaps today.',
    crumbs: [['Home', '/'], ['Key facts', '/facts/']],
    h1: 'Key facts',
    lead: 'Short, quotable facts about Ferrocast for humans and assistants. Sources: this repository, [the docs](/docs/), and [GitHub](https://github.com/drumst0ck/ferrocast).',
    factsH: 'At a glance',
    facts: [
      'Ferrocast is a fast, open-source keyboard launcher for macOS, written in Rust (MIT licence).',
      `Apple Silicon Macs only; Intel is not supported. The app declares macOS ${MIN_MACOS.version} (${MIN_MACOS.name}) or later as its minimum; screen recording needs macOS 14 or later.`,
      'About 8.5 MB on disk and one process at idle (measured on the maintainer’s Mac; see the home page method note).',
      `${c.pairs} local file conversions across ${c.inputs} input formats, plus ${c.tools} file tools and ${c.actions} file actions from a drag-and-drop wheel.`,
      'About 40 window-management commands, clipboard history, snippets, quicklinks, calculator, and floating Markdown notes.',
      'Ask AI uses Ollama on your Mac by default; you can point it at other providers. API keys go in the Keychain.',
      'Raycast-compatible Script Commands; snippets import from a Raycast JSON export.',
      'File conversions and file tools do not upload your files; results are written next to the originals.',
      'No account and no subscription tier.',
      'Install with the DMG (first open may need “Open Anyway” in System Settings) or Homebrew (builds on your Mac).',
      'Optional helpers when installed: ffmpeg, LibreOffice, Ghostscript, cwebp, and unrar.',
      'Not included today: an extension store, sync between Macs, React extensions, Search Menu Bar Items, and AI context from selection or screen.',
      'Ferrocast is independent and not affiliated with Raycast or Alfred.',
    ],
    linksH: 'Learn more',
    links: [
      ['Home and install', '/'],
      [`All ${c.pairs} conversions`, '/files/'],
      [`All ${c.tools} file tools`, '/tools/'],
      ['Free file converter for Mac (vs Permute and CloudConvert)', '/free-file-converter-mac/'],
      ['Migrating from Raycast', '/docs/getting-started/migrating-from-raycast/'],
      ['Raycast alternative: Ferrocast vs Raycast vs Alfred', '/raycast-alternative/'],
      ['Ferrocast vs Raycast: what is free, what is paid', '/ferrocast-vs-raycast/'],
      ['Run Raycast Script Commands', '/raycast-script-commands/'],
      ['Free Alfred alternative', '/alfred-alternative/'],
      ['Data and privacy', '/docs/reference/data-and-privacy/'],
      ['Changelog', '/changelog/'],
    ],
    faqH: 'Frequently asked questions',
  };
}
