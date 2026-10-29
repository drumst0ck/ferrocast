/*
 * The guide and comparison pages, grouped in four blocks. One list feeds the footer "Guides" column,
 * the home page "Compare and guides" section and the "Related" block at the end of each page, so a new
 * page is added here once. Labels are the short form of each page's H1; paths must exist (see guides.test.ts).
 */
export type GuideLang = 'en' | 'es';

export interface Guide { path: string; label: string; featured?: boolean }
export interface GuideBlock { id: string; title: string; guides: Guide[] }

const g = (path: string, label: string, featured = false): Guide => ({ path, label, ...(featured ? { featured } : {}) });

const blocks: Record<GuideLang, GuideBlock[]> = {
  en: [
    { id: 'launcher', title: 'Launcher and alternatives', guides: [
      g('/raycast-alternative/', 'Free Raycast alternative for Mac', true),
      g('/ferrocast-vs-raycast/', 'Ferrocast vs Raycast: free vs paid', true),
      g('/alfred-alternative/', 'Free Alfred alternative', true),
      g('/raycast-script-commands/', 'Run Raycast Script Commands without Raycast'),
      g('/local-ai-launcher/', 'Mac launcher with local AI'),
      g('/mac-window-management/', 'Mac window management: 40 commands'),
    ] },
    { id: 'text', title: 'Text and clipboard', guides: [
      g('/clipboard-manager-mac/', 'Free clipboard manager for Mac', true),
      g('/text-expander-mac/', 'Free text expander for Mac'),
      g('/translate-selected-text-mac/', 'Translate selected text on Mac'),
    ] },
    { id: 'capture', title: 'Capture and video', guides: [
      g('/cleanshot-alternative/', 'Free CleanShot X alternative', true),
      g('/screen-studio-alternative/', 'Free Screen Studio alternative'),
      g('/blur-video-mac/', 'Blur part of a video on Mac'),
      g('/compress-video-to-size/', 'Compress a video to a target size'),
    ] },
    { id: 'files', title: 'Files', guides: [
      g('/free-file-converter-mac/', 'Free file converter for Mac', true),
      g('/remove-metadata-mac/', 'Remove location and metadata on Mac'),
      g('/tools/organize-pdf/', 'Organize PDF pages on Mac'),
      g('/files/', 'All conversions and file tools'),
    ] },
  ],
  es: [
    { id: 'launcher', title: 'Lanzador y alternativas', guides: [
      g('/es/alternativa-a-raycast/', 'Alternativa gratis a Raycast para Mac', true),
      g('/es/raycast-vs-alfred-vs-spotlight/', 'Raycast, Alfred o Spotlight: cuál elegir', true),
      g('/es/alternativa-a-alfred/', 'Alternativa gratis a Alfred', true),
      g('/es/ia-local-mac/', 'Lanzador para Mac con IA local'),
      g('/es/gestion-de-ventanas-mac/', 'Organizar ventanas en Mac: 40 comandos'),
    ] },
    { id: 'text', title: 'Texto y portapapeles', guides: [
      g('/es/historial-portapapeles-mac/', 'Historial del portapapeles en Mac', true),
      g('/es/expansor-de-texto-mac/', 'Expansor de texto gratis para Mac'),
      g('/es/traducir-texto-seleccionado-mac/', 'Traducir el texto seleccionado en Mac'),
    ] },
    { id: 'capture', title: 'Captura y vídeo', guides: [
      g('/es/captura-de-pantalla-con-scroll-mac/', 'Captura de pantalla con scroll en Mac', true),
      g('/es/grabar-pantalla-mac-zoom-automatico/', 'Grabar la pantalla del Mac con zoom automático'),
      g('/es/difuminar-video-mac/', 'Difuminar parte de un vídeo en Mac'),
      g('/es/comprimir-video-a-un-tamano/', 'Comprimir un vídeo a un tamaño concreto'),
      g('/es/editar-video-mac-sin-imovie/', 'Editar un vídeo en Mac sin iMovie'),
    ] },
    { id: 'files', title: 'Archivos', guides: [
      g('/es/convertidor-de-archivos-gratis-mac/', 'Convertidor de archivos gratis para Mac', true),
      g('/es/quitar-metadatos-mac/', 'Quitar ubicación y metadatos en Mac'),
      g('/es/unir-pdf-mac/', 'Unir, separar y reordenar PDF en Mac'),
      g('/es/convertir/', 'Convertir archivos en Mac'),
      g('/es/files/', 'Convertir y herramientas de archivo'),
    ] },
  ],
};

const facts: Record<GuideLang, Guide> = {
  en: g('/facts/', 'Key facts'),
  es: g('/es/facts/', 'Datos clave'),
};

export const guideBlocks = (lang: GuideLang): GuideBlock[] => blocks[lang];
export const factsGuide = (lang: GuideLang): Guide => facts[lang];
export const featuredGuides = (lang: GuideLang): Guide[] => blocks[lang].flatMap((b) => b.guides.filter((x) => x.featured));
export const allGuidePaths = (): string[] => (['en', 'es'] as const).flatMap((l) => [...blocks[l].flatMap((b) => b.guides.map((x) => x.path)), facts[l].path]);

/**
 * Up to `max` sibling guides from the same block (same language) for the "Related" block of `path`.
 * Siblings the page body already links to (`body` is the page copy; `/path/` inside it counts as a link) come last,
 * so they are only repeated when the block is too small to fill the list otherwise.
 */
export function relatedGuides(path: string, lang: GuideLang, body = '', max = 4): Guide[] {
  const block = blocks[lang].find((b) => b.guides.some((x) => x.path === path));
  if (!block) return [];
  const siblings = block.guides.filter((x) => x.path !== path);
  const linked = (x: Guide) => body.includes(`](${x.path})`) || body.includes(`href="${x.path}"`);
  const fresh = siblings.filter((x) => !linked(x));
  const repeated = siblings.filter(linked);
  const wanted = Math.min(3, siblings.length);
  return (fresh.length >= wanted ? fresh : [...fresh, ...repeated]).slice(0, max);
}
