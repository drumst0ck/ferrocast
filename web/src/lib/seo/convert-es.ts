/*
 * Spanish pages for the most searched conversions (/es/convertir/ and /es/convertir/<slug>/).
 * Source: the Notion brief "Brief: Hub /es/convertir (ES)". Engine facts come from
 * crates/core/src/convert.rs (sips for images, avconvert PresetHighestQuality for MOV to MP4,
 * ffmpeg "fps=12,scale=640:-1" for GIF, LibreOffice for PDF to DOCX). The macOS alternatives come from
 * Apple Support (Finder quick action Convert Image, Preview export). Never promise kept metadata or
 * quality that was not checked.
 */
import { allPairs, type Pair } from './convert';

export interface EsPair {
  slug: string;
  /** English pair slug (/convert/<en>/), used for hreflang and the data. */
  en: string;
  inLabel: string;
  outLabel: string;
  /** What the steps call the input ("el archivo HEIC", "el vídeo"). */
  thing?: string;
  title: string;
  description: string;
  h1: string;
  answer: string;
  engine: string;
  needs: string | null;
  notes: string[];
  /** Built-in macOS alternative, only when an Apple page documents it. */
  builtIn?: string;
  faq: [string, string][];
}

const FINDER = 'https://support.apple.com/es-es/guide/mac-help/mchl97ff9142/mac';
const PREVIEW = 'https://support.apple.com/es-es/guide/preview/prvw1012/mac';

export const esPairs: EsPair[] = [
  {
    slug: 'heic-a-jpg',
    en: 'heic-to-jpg',
    inLabel: 'HEIC',
    outLabel: 'JPG',
    title: 'Convertir HEIC a JPG en Mac, gratis y sin subir fotos | Ferrocast',
    description: 'Convierte fotos HEIC del iPhone a JPG en tu Mac sin subirlas. Ferrocast usa sips de macOS: arrastra las fotos, pulsa ⇧ y elige JPEG. Gratis y de código abierto.',
    h1: 'Convertir HEIC a JPG en tu Mac, sin subir las fotos',
    answer: 'Arrastra las fotos HEIC, pulsa ⇧ para abrir la rueda de conversión y suelta sobre JPEG. Ferrocast usa sips, la herramienta de imágenes de macOS, así que no hay que instalar nada más y las fotos no salen de tu Mac.',
    engine: 'sips (incluido en macOS)',
    needs: null,
    notes: [
      'Puedes convertir varias fotos a la vez: cada JPG se guarda junto a su original.',
      'El original HEIC no se toca ni se sobrescribe.',
    ],
    builtIn: `Sin instalar nada, macOS también lo hace: en el Finder, Control-clic → Acciones rápidas → Convertir imagen, o en Vista Previa con Archivo → Exportar ([Finder](${FINDER}), [Vista Previa](${PREVIEW})). Ferrocast añade la rueda de arrastrar y soltar y las otras 274 conversiones.`,
    faq: [
      ['¿Se suben mis fotos a algún servidor?', 'No. Ferrocast convierte las fotos en tu Mac con sips; no se sube nada.'],
      ['¿Puedo convertir muchas fotos HEIC a la vez?', 'Sí. Selecciona o arrastra varias fotos y elige JPEG: cada una se guarda como JPG junto a su original.'],
      ['¿Necesito instalar algo además de Ferrocast?', 'No. La conversión de HEIC a JPG usa herramientas que ya vienen con macOS.'],
    ],
  },
  {
    slug: 'webp-a-jpg',
    en: 'webp-to-jpg',
    inLabel: 'WebP',
    outLabel: 'JPG',
    title: 'Convertir WebP a JPG en Mac sin subir la imagen | Ferrocast',
    description: 'Convierte imágenes WebP descargadas de la web a JPG en tu Mac, sin subirlas. Ferrocast usa sips de macOS: arrastra, pulsa ⇧ y elige JPEG. Gratis y MIT.',
    h1: 'Convertir WebP a JPG en tu Mac, sin subir la imagen',
    answer: 'Arrastra la imagen WebP, pulsa ⇧ y suelta sobre JPEG. Ferrocast usa sips de macOS para leer el WebP y escribir el JPG, sin instalar nada más y sin subir el archivo.',
    engine: 'sips (incluido en macOS)',
    needs: null,
    notes: [
      'Leer WebP no necesita nada extra. Lo que sí necesita cwebp es el camino contrario, de JPG o PNG a WebP.',
      'El JPG se guarda junto al original, que no se modifica.',
    ],
    builtIn: `Vista Previa de macOS también puede exportar a JPEG con Archivo → Exportar ([Apple](${PREVIEW})). Ferrocast lo hace desde el lanzador o arrastrando, y en lote.`,
    faq: [
      ['¿Por qué las imágenes de la web vienen en WebP?', 'WebP es un formato de imagen pensado para la web que muchas páginas usan para que carguen más rápido. Algunas apps esperan JPG, de ahí la conversión.'],
      ['¿Necesito instalar algo?', 'No para pasar de WebP a JPG: Ferrocast usa sips, que viene con macOS. Solo para crear WebP hace falta cwebp (brew install webp).'],
      ['¿Se sube la imagen?', 'No. La conversión se hace en tu Mac.'],
    ],
  },
  {
    slug: 'mov-a-mp4',
    en: 'mov-to-mp4',
    inLabel: 'MOV',
    outLabel: 'MP4',
    title: 'Convertir MOV a MP4 en Mac, gratis y sin subir el vídeo | Ferrocast',
    description: 'Convierte un vídeo MOV a MP4 en tu Mac sin subirlo. Ferrocast usa avconvert de macOS con el ajuste de máxima calidad: arrastra, pulsa ⇧ y elige MP4. Gratis.',
    h1: 'Convertir MOV a MP4 en tu Mac, sin subir el vídeo',
    answer: 'Arrastra el vídeo MOV, pulsa ⇧ y suelta sobre MP4. Ferrocast usa avconvert, la herramienta de vídeo de macOS, con su ajuste de máxima calidad. No hace falta ffmpeg y el vídeo no sale de tu Mac.',
    engine: 'avconvert de macOS, ajuste PresetHighestQuality',
    needs: null,
    notes: [
      'Es una recodificación con el ajuste de máxima calidad de macOS, así que el MP4 puede ocupar más o menos que el MOV según el vídeo. Si lo que quieres es que ocupe menos, usa [comprimir un vídeo a un tamaño](/es/comprimir-video-a-un-tamano/).',
      'El MP4 se guarda junto al MOV, que no se modifica.',
    ],
    faq: [
      ['¿Necesito ffmpeg para pasar de MOV a MP4?', 'No. Ferrocast usa avconvert, que viene con macOS. ffmpeg solo hace falta para formatos que macOS no escribe, como MKV o WebM.'],
      ['¿Pierde calidad el vídeo?', 'Ferrocast usa el ajuste de máxima calidad de macOS (PresetHighestQuality). Cualquier recodificación cambia el archivo, pero ese ajuste prioriza la calidad sobre el tamaño.'],
      ['¿Se sube el vídeo?', 'No. Todo ocurre en tu Mac.'],
    ],
  },
  {
    slug: 'video-a-gif',
    en: 'mov-to-gif',
    inLabel: 'vídeo',
    outLabel: 'GIF',
    thing: 'el vídeo',
    title: 'Convertir un vídeo a GIF en Mac (MOV o MP4), sin subirlo | Ferrocast',
    description: 'Convierte un vídeo MOV o MP4, como una grabación de pantalla, a GIF en tu Mac sin subirlo. Ferrocast usa ffmpeg: 12 fps y 640 px de ancho. Gratis y MIT.',
    h1: 'Convertir un vídeo a GIF en tu Mac, sin subirlo',
    answer: 'Arrastra el vídeo (MOV o MP4), pulsa ⇧ y suelta sobre GIF. Ferrocast usa ffmpeg y crea un GIF a 12 fotogramas por segundo y 640 píxeles de ancho, junto al vídeo original.',
    engine: 'ffmpeg, a 12 fps y 640 px de ancho',
    needs: 'ffmpeg',
    notes: [
      'El GIF sale siempre a 12 fps y 640 px de ancho (la altura se ajusta para mantener la proporción). Es un tamaño pensado para compartir, no para conservar todo el detalle.',
      'GIF admite pocos colores, así que los degradados y los vídeos con mucha variedad de color pierden calidad. Para pantallas con texto suele quedar bien.',
      'Si quieres recortar el vídeo antes, usa [Trim video](/tools/trim-video/).',
    ],
    faq: [
      ['¿Necesito ffmpeg para convertir un vídeo a GIF?', 'Sí. Instálalo con brew install ffmpeg. Si falta, Ferrocast te muestra el comando.'],
      ['¿Puedo elegir los fotogramas por segundo o el tamaño?', 'No en esta conversión: el GIF sale a 12 fps y 640 px de ancho.'],
      ['¿Funciona con una grabación de pantalla del Mac?', 'Sí. Las grabaciones de pantalla de macOS son .mov, y MOV a GIF es una de las conversiones de Ferrocast.'],
    ],
  },
  {
    slug: 'pdf-a-word',
    en: 'pdf-to-docx',
    inLabel: 'PDF',
    outLabel: 'Word',
    title: 'Convertir PDF a Word (DOCX) en Mac, gratis y sin subirlo | Ferrocast',
    description: 'Convierte un PDF a Word (.docx) en tu Mac sin subirlo a ninguna web. Ferrocast usa LibreOffice, gratis y opcional: arrastra el PDF, pulsa ⇧ y elige DOCX.',
    h1: 'Convertir PDF a Word en tu Mac, sin subir el archivo',
    answer: 'Arrastra el PDF, pulsa ⇧ y suelta sobre DOCX. Ferrocast usa LibreOffice, gratuito y de código abierto, para crear un documento de Word junto al PDF. El archivo no se sube a ningún sitio.',
    engine: 'LibreOffice (opcional y gratuito)',
    needs: 'LibreOffice',
    notes: [
      'Hace falta LibreOffice. Si no está instalado, Ferrocast te muestra el comando: brew install --cask libreoffice.',
      'El resultado depende del PDF: los documentos con un diseño sencillo se convierten mejor que los que tienen columnas, tablas complejas o mucha maquetación. Revisa el documento antes de enviarlo.',
      'El .docx se guarda junto al PDF, que no se modifica.',
    ],
    faq: [
      ['¿Se sube mi PDF a algún servidor?', 'No. LibreOffice convierte el PDF en tu Mac; no se sube nada. Es la diferencia con los convertidores web.'],
      ['¿Necesito Microsoft Word?', 'No para convertir: Ferrocast usa LibreOffice. Para abrir el .docx puedes usar Word, Pages o LibreOffice.'],
      ['¿Necesito instalar algo?', 'Sí, LibreOffice: brew install --cask libreoffice. Es gratuito.'],
    ],
  },
];

export const hub = {
  path: '/es/convertir/',
  title: 'Convertir archivos en Mac sin subirlos: HEIC, MOV, PDF y más | Ferrocast',
  description: 'Convierte archivos en tu Mac sin subirlos a ninguna web: HEIC a JPG, MOV a MP4, vídeo a GIF, PDF a Word y 275 conversiones más. Gratis y de código abierto.',
  h1: 'Convertir archivos en tu Mac, sin subirlos',
  lead: 'Ferrocast convierte archivos en tu propio Mac: arrastras el archivo, pulsas ⇧ y eliges el formato. El resultado se guarda junto al original y nunca lo sobrescribe. Son 275 conversiones entre 46 formatos de entrada; primero usa herramientas de macOS y, para algunos formatos, ffmpeg, LibreOffice o cwebp si los tienes instalados.',
};

const PAIRS = new Map(allPairs().map((p) => [p.slug, p]));

/** The English pair behind a Spanish page; throws at build time if the data drifts. */
export function pairOf(e: EsPair): Pair {
  const p = PAIRS.get(e.en);
  if (!p) throw new Error(`no English pair ${e.en} for /es/convertir/${e.slug}/`);
  if ((p.needs ?? null) !== e.needs) throw new Error(`${e.slug}: needs ${e.needs} but the data says ${p.needs}`);
  return p;
}

/** Spanish page for an English pair slug, if there is one (for hreflang on /convert/<slug>/). */
export function esFor(enSlug: string): EsPair | undefined {
  return esPairs.find((e) => e.en === enSlug);
}
