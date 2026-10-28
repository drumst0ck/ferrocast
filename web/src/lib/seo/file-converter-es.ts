/*
 * Copy for /es/convertidor-de-archivos-gratis-mac/, the Spanish pair of /free-file-converter-mac/.
 * Ferrocast counts come from data/status.ts. Optional-tool lines come from
 * src/content/docs/docs/files/optional-tools.md. Shortcuts and “next to the original” come from
 * the Spanish conversion reference. Permute and CloudConvert cells were read on CHECKED from
 * software.charliemonroe.net/permute/ and cloudconvert.com/pricing.
 * Camomilla, AltFormr, The Converter and nxtconvert are left out: they were not opened.
 * iCloud sync is not mentioned. The Spanish blur guide is linked now that it is published.
 */
import type { GuideCopy } from './compress-video';
import { counts } from '../../data/status';

export const CHECKED = '2026-10-28';

const c = counts();
const PERMUTE = 'https://software.charliemonroe.net/permute/';
const CLOUD = 'https://cloudconvert.com/pricing';

/** The first two sections render as H3 under the how-to, matching the brief. */
export const H3_COUNT = 2;

/** Inserted after the “¿Hace falta instalar algo?” section. */
export const OPTIONAL_H = '¿Hace falta instalar algo?';

export const optionalTools: { cols: [string, string, string]; rows: [string, string, string][] } = {
  cols: ['Herramienta', 'Para qué', 'Línea que muestra Ferrocast'],
  rows: [
    ['ffmpeg', 'Audio y vídeo, y las conversiones de audio y vídeo que lo necesitan', '`brew install ffmpeg`'],
    ['LibreOffice', 'Documentos, hojas de cálculo, presentaciones y diapositivas a imagen', '`brew install --cask libreoffice`'],
    ['cwebp', 'Escribir WebP', '`brew install webp`'],
    ['Ghostscript', 'Comprimir PDF. No hay otra vía', '`brew install ghostscript`'],
    ['unrar', 'Extraer RAR si el tar del sistema no puede', '`brew install --cask rar`'],
  ],
};

export const page: GuideCopy = {
  lang: 'es',
  path: '/es/convertidor-de-archivos-gratis-mac/',
  title: 'Convertidor de archivos gratis para Mac, sin subir nada | Ferrocast',
  description:
    'Convierte imágenes, vídeo, audio, PDF y documentos en tu Mac sin subirlos: 275 conversiones y 26 herramientas, gratis y MIT. Comparado con Permute y CloudConvert.',
  crumbs: [
    ['Inicio', '/es/'],
    ['Convertir', '/es/convertir/'],
    ['Convertidor de archivos gratis', '/es/convertidor-de-archivos-gratis-mac/'],
  ],
  h1: 'Un convertidor de archivos gratis para Mac que no sube tus archivos',
  lead: [
    `Ferrocast convierte archivos en tu Mac: arrastras, pulsas ⇧ y sueltas sobre un formato. Son ${c.pairs} conversiones entre ${c.inputs} formatos de entrada, y ${c.tools} herramientas (comprimir, recortar, tapar zonas, unir PDF). El resultado se guarda junto al original. Es gratis, de código abierto (MIT) y no pide cuenta.`,
    'Permute es un convertidor de pago para Mac, con narración, OCR y transcripción. CloudConvert es una web, con un plan gratis diario. La tabla dice lo que publica cada web, con la fecha en que lo leí.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · macOS 13 o posterior · sin cuenta' },
  limits: {
    h: 'Ferrocast, Permute y CloudConvert: qué dice cada web',
    caption: 'Ferrocast, Permute 4 y CloudConvert, con datos de sus webs el 28 de octubre de 2026',
    cols: ['', 'Ferrocast', 'Permute 4', 'CloudConvert'],
    rows: [
      ['Dónde se ejecuta', 'En tu Mac', 'En tu Mac', 'Servicio web'],
      [
        'Precio',
        'Gratis, sin plan de pago',
        `14,99 $ de una vez en la web del autor y en la Mac App Store. Setapp lo ofrece a 14,99 $ al mes. El precio de lista está en dólares sin IVA; la base es el euro y el dólar puede moverse. [Permute](${PERMUTE}).`,
        `Plan gratis: 10 conversiones al día. Paquete: 17 $ por 1000 créditos, que no caducan. Suscripción: 9 $ al mes por 1000 créditos al mes; los que sobran no pasan al mes siguiente. [Precios](${CLOUD}).`,
      ],
      [
        'Formatos',
        `${c.pairs} conversiones entre ${c.inputs} formatos de entrada, en nueve familias: imágenes, audio, vídeo, PDF, documentos, presentaciones, hojas de cálculo, subtítulos y comprimidos`,
        '«100+ formats»: vídeo, audio, imágenes, PDF y texto',
        'No lo comparo aquí',
      ],
      [
        'Extras',
        `${c.tools} herramientas de archivo, en la misma app que el lanzador`,
        'Narración de texto a voz, OCR de imagen a texto, transcripción a texto o SRT en el propio Mac, y exportar páginas de PDF como imágenes',
        'No lo comparo aquí',
      ],
      [
        'Límites',
        'Algunos pares necesitan ffmpeg, LibreOffice, cwebp, Ghostscript o unrar. Si falta la herramienta, Ferrocast muestra el comando y atenúa ese formato en la rueda',
        'La web avisa de que algunas funciones de la versión de la App Store son distintas. No detalla cuáles en la página de inicio',
        'En el plan gratis: 1 GB por archivo, 5 minutos de proceso y 5 tareas a la vez. Una conversión larga puede gastar más de un crédito',
      ],
      [
        'Requisitos',
        'Apple Silicon, macOS 13 o posterior',
        'Permute 4: macOS 26 y Apple Silicon. Permute 3: macOS 11–15 o un Mac Intel, licencia permanente, con paso gratis a Permute 4',
        'Un navegador',
      ],
      ['Código', 'MIT', 'No indica una licencia abierta', 'No indica una licencia abierta'],
    ],
    note: `Datos de [Permute](${PERMUTE}) y de [los precios de CloudConvert](${CLOUD}), leídos el 28 de octubre de 2026. Ferrocast, de la [referencia de conversiones](/es/docs/files/conversions/). Si algo cambió, avísanos. Ferrocast es independiente y no está afiliado a Charlie Monroe Software ni a CloudConvert.`,
  },
  steps: {
    h: 'Cómo convertir un archivo',
    howToName: 'Convertir un archivo en Mac con Ferrocast, sin subirlo',
    items: [
      ['Instala Ferrocast', 'Descarga el DMG o instálalo con Homebrew.'],
      ['Arrastra el archivo', 'Empieza a arrastrar uno o varios archivos desde el Finder, o selecciónalos en el lanzador.'],
      ['Pulsa ⇧', 'Se abre la rueda con los formatos a los que puede pasar ese archivo. También puedes pulsar ⌘T sobre un resultado de archivo, ejecutar Convert Files, o pulsar ⌥↵ sobre el archivo.'],
      ['Suelta sobre el formato', 'El resultado se escribe junto al original y nunca lo sobrescribe.'],
    ],
  },
  sections: [
    {
      h: 'Varios archivos a la vez',
      paras: ['Sí. Arrastra o selecciona varios y suéltalos sobre un formato. Cada uno se convierte junto a su original.'],
    },
    {
      h: 'Dónde queda el resultado',
      paras: ['Junto al original, con otro nombre. Ferrocast no pisa el archivo del que partiste. Puedes borrar la copia si no te sirve y el original sigue ahí.'],
    },
    {
      h: 'Qué convierte',
      paras: [
        `Nueve familias y ${c.pairs} pares. La lista completa, con la herramienta que usa cada par, está en la [referencia de conversiones](/es/docs/files/conversions/). Un destino seguido del nombre de una herramienta necesita esa herramienta; el resto usa lo que trae macOS (sips, afconvert, avconvert, textutil, ditto, tar).`,
      ],
    },
    {
      h: 'Las más buscadas',
      list: [
        '[Convertir HEIC a JPG](/es/convertir/heic-a-jpg/) · [Convertir WebP a JPG](/es/convertir/webp-a-jpg/)',
        '[Convertir MOV a MP4](/es/convertir/mov-a-mp4/) · [Convertir un vídeo a GIF](/es/convertir/video-a-gif/)',
        '[Convertir PDF a Word](/es/convertir/pdf-a-word/)',
        '[El hub de convertir](/es/convertir/) · [las 275 conversiones](/es/files/)',
      ],
    },
    {
      h: OPTIONAL_H,
      paras: [
        'Primero se usan herramientas que trae macOS. Cinco herramientas opcionales abren el resto. Si falta una, Ferrocast muestra la línea de instalación en vez de fallar en silencio, y en la rueda esos formatos aparecen atenuados con el comando. La [página de herramientas opcionales](/docs/files/optional-tools/) está en inglés.',
      ],
    },
    {
      h: 'Más que un convertidor: 26 herramientas',
      paras: [
        `Además de convertir, hay ${c.tools} herramientas en la rueda de ⌘: comprimir un vídeo a un tamaño, recortar, unir o dividir PDF, organizar páginas dentro de un PDF, y tapar zonas de una foto o de un vídeo. El resultado también se guarda junto al original.`,
        'En español: [comprimir un vídeo a un tamaño](/es/comprimir-video-a-un-tamano/), [difuminar parte de un vídeo](/es/difuminar-video-mac/) y [todas las herramientas](/es/tools/). En inglés: [unir PDF](/tools/merge-pdf/), [dividir PDF](/tools/split-pdf/), [organizar páginas](/tools/organize-pdf/), [tapar zonas de una foto](/tools/redact-image/) y [tapar zonas de un vídeo](/tools/redact-video/).',
      ],
    },
    {
      h: 'En qué es mejor Permute (y cuándo usar CloudConvert)',
      paras: [
        'Permute, según su web, añade narración de texto a voz, OCR de una imagen a texto y transcripción de audio a texto o a SRT, hechas en el Mac. También exporta páginas de PDF como imágenes. Permute 3 funciona en un Mac Intel o en macOS 11 a 15. Ferrocast no hace narración, ni OCR, ni transcripción, y solo funciona en Apple Silicon. Si necesitas alguna de esas tres, yo usaría Permute.',
        'CloudConvert no se instala y sirve en cualquier ordenador. El plan gratis son 10 conversiones al día, con archivos de hasta 1 GB y 5 minutos de proceso. Si el archivo no puede salir de tu Mac, no te sirve: el archivo se sube a su servicio. Si solo quieres convertir una vez y el archivo no es privado, puede bastar.',
      ],
    },
    {
      h: 'Requisitos y seguridad',
      paras: [
        'Apple Silicon y macOS 13 o posterior. No funciona en un Mac Intel. Unos 8,5 MB en disco, medidos en el Mac del autor; el método está en [Datos clave](/es/facts/). Licencia MIT, sin cuenta.',
        'La instalación, con DMG o Homebrew, está en la [guía de instalación](/es/docs/getting-started/install/).',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      [
        '¿Hay un convertidor de archivos gratis para Mac que no suba los archivos?',
        `Sí. Ferrocast convierte en tu Mac ${c.pairs} pares de formatos (imágenes, vídeo, audio, PDF, documentos, presentaciones, hojas de cálculo, subtítulos y comprimidos) y guarda el resultado junto al original. Es gratis, de código abierto (MIT) y no pide cuenta.`,
      ],
      [
        '¿Necesito ffmpeg para convertir archivos con Ferrocast?',
        'No para muchas conversiones: primero usa herramientas que trae macOS, como sips o avconvert. Algunos pares necesitan ffmpeg, LibreOffice, cwebp, Ghostscript o unrar; en ese caso Ferrocast muestra el comando de instalación.',
      ],
      [
        '¿En qué es mejor Permute?',
        'Según su web, Permute añade narración de texto a voz, OCR y transcripción a SRT, y Permute 3 funciona en Mac con Intel. Ferrocast solo funciona en Apple Silicon.',
      ],
      ['¿Puedo convertir varios archivos a la vez?', 'Sí. Arrastra o selecciona varios y suéltalos sobre un formato. Cada archivo se convierte junto a su original.'],
      ['¿Sobrescribe el original?', 'No. El resultado se guarda junto al original y nunca lo sobrescribe.'],
      [
        '¿Cuánto cuesta CloudConvert?',
        'El plan gratis son 10 conversiones al día, archivos de hasta 1 GB y 5 minutos de proceso. Un paquete cuesta 17 $ por 1000 créditos. La suscripción son 9 $ al mes por 1000 créditos al mes. Lo leí en su página de precios el 28 de octubre de 2026.',
      ],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Convertir archivos', '/es/convertir/'],
      ['Difuminar parte de un vídeo', '/es/difuminar-video-mac/'],
      ['Las 275 conversiones', '/es/files/'],
      ['Herramientas de archivo', '/es/tools/'],
      ['Referencia de conversiones', '/es/docs/files/conversions/'],
      ['Herramientas opcionales (en inglés)', '/docs/files/optional-tools/'],
      ['Datos clave', '/es/facts/'],
      ['Instalar', '/es/docs/getting-started/install/'],
      ['Free file converter (English)', '/free-file-converter-mac/'],
    ],
  },
};

if (page.description.length < 140) throw new Error(`converter ES description is ${page.description.length}`);
if (c.pairs !== 275 || c.inputs !== 46 || c.tools !== 26) {
  throw new Error(`converter ES brief counts drifted: ${c.pairs}/${c.inputs}/${c.tools}`);
}
