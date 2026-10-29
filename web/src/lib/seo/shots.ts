/*
 * Existing product screenshots (public/img/) placed in the SEO guides, from the brief "Imágenes en guías" (29 Oct 2026),
 * table §1. Only screenshots that already exist are used; a page the table has no screenshot for gets none and is listed
 * in PAGES_WITHOUT_SHOT. width/height are the real pixel size of each file (checked by shots.test.ts) so the layout does
 * not shift. Pages render them through components/seo/Shots.astro.
 */

export interface ShotFile { w: number; h: number; en: string; es: string }
export type ShotPlace = 'lead' | 'steps';
export interface PageShot { file: string; src: string; w: number; h: number; alt: string; where: ShotPlace }

export const SHOTS: Record<string, ShotFile> = {
  search: { w: 1300, h: 823, en: 'Ferrocast launcher answering "100 usd in eur" with 88.87 EUR, above web, file, emoji and Ask AI fallbacks', es: 'El lanzador de Ferrocast responde «100 usd in eur» con 88.87 EUR, sobre las opciones de búsqueda web, archivos, emoji y Ask AI' },
  panel: { w: 1300, h: 823, en: 'Ferrocast search results for "safari" with the actions menu open: Open Application, Show in Finder, Copy Path and Set Hotkey', es: 'Resultados de Ferrocast para «safari» con el menú de acciones abierto: Open Application, Show in Finder, Copy Path y Set Hotkey' },
  settings: { w: 1300, h: 823, en: 'Ferrocast Settings, General tab: launcher hotkey, appearance, compact mode, launch at login and menu bar icon', es: 'Ajustes de Ferrocast, pestaña General: atajo del lanzador, apariencia, modo compacto, inicio al arrancar y icono de la barra de menús' },
  clipboard: { w: 1300, h: 823, en: 'Ferrocast clipboard history with a pinned link, earlier copies and a preview showing type, source app, times copied and length', es: 'Historial del portapapeles de Ferrocast con un enlace fijado, copias anteriores y una vista previa con tipo, app de origen, veces copiado y longitud' },
  snippets: { w: 1300, h: 823, en: 'Ferrocast snippets list with Shrug, Today\'s date and Console log, and the keyword and length of the selected snippet', es: 'Lista de snippets de Ferrocast con Shrug, Today\'s date y Console log, y la palabra clave y la longitud del snippet seleccionado' },
  ai: { w: 1300, h: 823, en: 'Ferrocast Ask AI window using a local llama3.2 model, with two questions about frecency and their answers', es: 'La ventana Ask AI de Ferrocast con un modelo local llama3.2, con dos preguntas sobre frecency y sus respuestas' },
  'record-editor': { w: 1500, h: 954, en: 'The Ferrocast recording editor: preview, inspector with three auto zooms, and the timeline', es: 'El editor de grabaciones de Ferrocast: vista previa, inspector con tres zooms automáticos y línea de tiempo' },
  'files-annotate': { w: 1500, h: 950, en: 'Annotate Image window in Ferrocast with an arrow, a box, a numbered step, a callout, a text label and a magnifier on an image', es: 'La ventana Annotate Image de Ferrocast con una flecha, una caja, un número, una llamada, una etiqueta de texto y una lupa sobre una imagen' },
  'files-vredact': { w: 1500, h: 950, en: 'Redact Video window in Ferrocast with three numbered cover zones over a frame, a zone list with time ranges, and Solid and Blur modes', es: 'La ventana Redact Video de Ferrocast con tres zonas de cobertura numeradas sobre un fotograma, una lista de zonas con intervalos y modos Solid y Blur' },
  'files-redact': { w: 1500, h: 950, en: 'Redact Image window in Ferrocast with two cover zones over an image, Solid and Blur modes and colour options', es: 'La ventana Redact Image de Ferrocast con dos zonas de cobertura sobre una imagen, modos Solid y Blur y opciones de color' },
  'files-compress-video': { w: 1500, h: 950, en: 'Compress Audio and Video window in Ferrocast for an MP4, with a target size and the bitrates it would use', es: 'La ventana Compress Audio and Video de Ferrocast para un MP4, con un tamaño objetivo y las tasas de bits que usaría' },
  'files-vtrim': { w: 1500, h: 950, en: 'Trim Video window in Ferrocast with start and end markers on a timeline and the clip duration', es: 'La ventana Trim Video de Ferrocast con marcas de inicio y fin en una línea de tiempo y la duración del clip' },
  'files-vsplit': { w: 1500, h: 950, en: 'Split Video window in Ferrocast with two cut marks on a timeline that give three clips', es: 'La ventana Split Video de Ferrocast con dos marcas de corte en una línea de tiempo que dan tres clips' },
  'files-vspeed': { w: 1500, h: 950, en: 'Change Video Speed window in Ferrocast with speeds from 0.25× to 4× and the duration going from 6 seconds to 3 seconds', es: 'La ventana Change Video Speed de Ferrocast con velocidades de 0.25× a 4× y una duración que pasa de 6 a 3 segundos' },
  'files-meta': { w: 1500, h: 950, en: 'Edit Metadata window in Ferrocast with the artist, comment and title tags of a video, delete buttons and a Remove all metadata option', es: 'La ventana Edit Metadata de Ferrocast con las etiquetas artist, comment y title de un vídeo, botones para borrarlas y la opción Remove all metadata' },
  'files-wheel-convert': { w: 1500, h: 950, en: 'Ferrocast convert wheel for a PNG file, with one segment per target format and WebP highlighted', es: 'La rueda de conversión de Ferrocast para un PNG, con un segmento por formato de destino y WebP resaltado' },
  'files-wheel-tools': { w: 1500, h: 950, en: 'Ferrocast tools wheel for a PNG file, with Compress, Metadata, Edit Photo, Annotate, Background, Crop and Redact segments', es: 'La rueda de herramientas de Ferrocast para un PNG, con segmentos Compress, Metadata, Edit Photo, Annotate, Background, Crop y Redact' },
  'files-organize': { w: 1500, h: 950, en: 'Organize PDF window in Ferrocast with thumbnails of a three-page PDF, one page rotated and one duplicated', es: 'La ventana Organize PDF de Ferrocast con miniaturas de un PDF de tres páginas, una girada y otra duplicada' },
};

const LAUNCHER = ['search', 'panel', 'settings'];
const SCAN: Record<string, string[]> = {
  '/raycast-alternative/': LAUNCHER,
  '/es/alternativa-a-raycast/': LAUNCHER,
  '/ferrocast-vs-raycast/': LAUNCHER,
  '/alfred-alternative/': LAUNCHER,
  '/es/alternativa-a-alfred/': LAUNCHER,
  '/es/raycast-vs-alfred-vs-spotlight/': LAUNCHER,
  '/clipboard-manager-mac/': ['clipboard'],
  '/es/historial-portapapeles-mac/': ['clipboard'],
  '/text-expander-mac/': ['snippets'],
  '/es/expansor-de-texto-mac/': ['snippets'],
  '/local-ai-launcher/': ['ai'],
  '/es/ia-local-mac/': ['ai'],
  '/screen-studio-alternative/': ['record-editor'],
  '/es/grabar-pantalla-mac-zoom-automatico/': ['record-editor'],
  '/cleanshot-alternative/': ['files-annotate'],
  '/es/captura-de-pantalla-con-scroll-mac/': ['files-annotate'],
  // files-vredact is the figure BlurVideoPage already shows in the steps; files-redact goes after the lead.
  '/blur-video-mac/': ['files-redact', 'files-vredact'],
  '/es/difuminar-video-mac/': ['files-redact', 'files-vredact'],
  '/compress-video-to-size/': ['files-compress-video'],
  '/es/comprimir-video-a-un-tamano/': ['files-compress-video'],
  '/es/editar-video-mac-sin-imovie/': ['files-vtrim', 'files-vsplit', 'files-vspeed'],
  '/remove-metadata-mac/': ['files-meta'],
  '/es/quitar-metadatos-mac/': ['files-meta'],
  '/free-file-converter-mac/': ['files-wheel-convert', 'files-wheel-tools'],
  '/es/convertidor-de-archivos-gratis-mac/': ['files-wheel-convert', 'files-wheel-tools'],
  '/es/unir-pdf-mac/': ['files-organize'],
};

/** The table §1 pages this module fills (the /tools/<slug>/ pages and the /convert/* templates are covered below). */
export const TABLE_PAGES = Object.keys(SCAN);

/** Table has no suitable screenshot: nothing is placed and Jose gets the list in the PR. */
export const PAGES_WITHOUT_SHOT = [
  '/mac-window-management/',
  '/es/gestion-de-ventanas-mac/',
  '/translate-selected-text-mac/',
  '/es/traducir-texto-seleccionado-mac/',
  '/raycast-script-commands/',
];

const isConvertTemplate = (path: string) => /^\/(convert|es\/convertir)\/[^/]+\/$/.test(path) && path !== '/es/convertir/';

export function allShotPaths(): string[] {
  return [...TABLE_PAGES];
}

/** First screenshot goes after the lead, the rest into the steps section; the /convert/* templates get one, in «How to». */
export function shotsFor(path: string): PageShot[] {
  const lang = path.startsWith('/es/') ? 'es' : 'en';
  const files = isConvertTemplate(path) ? ['files-wheel-convert'] : SCAN[path] ?? [];
  return files.map((file, i) => {
    const s = SHOTS[file];
    return { file, src: `/img/${file}.jpg`, w: s.w, h: s.h, alt: s[lang], where: i === 0 && !isConvertTemplate(path) ? 'lead' : 'steps' };
  });
}
