/*
 * Copy for /es/unir-pdf-mac/ (Spanish only, board idea "Unir, separar y reordenar páginas de PDF en Mac sin
 * subirlos"). Ferrocast facts come from the code: crates/app/src/services.rs (FileTool::Merge on PDFs runs
 * macOS's own "Combine PDF Pages" join tool and writes "<name> merged.pdf"; SplitPages writes one PDF per page
 * through PDFKit as "<name> page N.pdf"), crates/app/src/app.rs (Convert view offers "Choose order…" for two or
 * more files of one family; the Tools wheel's Merge PDF runs straight away), crates/app/src/draw.rs (the Choose
 * Order window: Merge ↵, Move ⌘↑/↓), crates/app/src/route.rs and crates/core/src/compress.rs (Ghostscript
 * -dPDFSETTINGS /printer, /ebook, /screen; "<name> compressed.pdf"; a result that is not smaller is kept),
 * crates/macos/src/pdf.rs (the organizer refuses a locked PDF) and the organizer copy in organize-pdf.ts.
 * Preview facts were read on CHECKED on support.apple.com/es-es (Vista Previa guide). What Apple's pages do not
 * say is written as "no se describe" (never "no puede"). Rules: Vista Previa already does this for free and the
 * page says so first; no speed claims; no third-party apps (their pages were not opened).
 */
import type { GuideCopy } from './compress-video';

export const CHECKED = '2026-10-28';

const COMBINE = 'https://support.apple.com/es-es/guide/preview/prvw43696/mac';
const MOVE = 'https://support.apple.com/es-es/guide/preview/prvw11793/mac';
const REDUCE = 'https://support.apple.com/es-es/guide/preview/prvw1509/mac';

export const page: GuideCopy = {
  lang: 'es',
  path: '/es/unir-pdf-mac/',
  title: 'Unir, separar y reordenar PDF en Mac sin subirlos | Ferrocast',
  description:
    'Une varios PDF, sepáralos por páginas, reordénalos y comprime en tu Mac sin subirlos. Ferrocast es gratis y guarda una copia. Vista Previa también puede.',
  crumbs: [['Inicio', '/es/'], ['Archivos', '/es/files/'], ['Unir PDF', '/es/unir-pdf-mac/']],
  h1: 'Unir, separar y reordenar páginas de PDF en Mac sin subirlos',
  lead: [
    'Vista Previa, que ya viene con el Mac, une, reordena y borra páginas de un PDF. Ferrocast hace lo mismo en local con otro recorrido. Arrastras los PDF, tocas ⌘ y los sueltas sobre Merge PDF. Una ventana te deja elegir el orden y un organizador con miniaturas (⇧⌘O) reordena las páginas. Siempre se guarda una copia nueva junto al original. No se sube nada. Es gratis y de código abierto (MIT).',
    'Dos límites desde el principio: Ferrocast separa un PDF solo en una página por archivo (no por intervalos) y comprimir necesita Ghostscript, que no viene con el Mac. Escribo Ferrocast; no está afiliado a Apple.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · sin cuenta' },
  limits: {
    h: 'Ferrocast y Vista Previa, tarea por tarea',
    caption: 'Unir, separar, reordenar y comprimir PDF en Mac con Ferrocast y con Vista Previa',
    cols: ['', 'Ferrocast', 'Vista Previa (macOS)'],
    rows: [
      ['Precio', 'Gratis (MIT)', 'Incluida con macOS'],
      ['Unir PDF', 'Ventana «Choose order…» para decidir el orden (⌘↑ / ⌘↓ o arrastrando filas) y ↵; o Merge PDF en la rueda de herramientas, que une al instante. Escribe «… merged.pdf» con la herramienta de combinar PDF del propio macOS', 'Arrastras miniaturas de la barra lateral de un PDF a la de otro, o arrastras el icono de un PDF desde el Finder a la barra lateral'],
      ['Separar un PDF', 'Split PDF: un archivo por página («… page 1.pdf», «… page 2.pdf»…); no separa por intervalos', 'Apple describe crear un PDF nuevo arrastrando miniaturas desde la barra lateral al escritorio'],
      ['Reordenar, girar o borrar páginas', 'Organizador (⇧⌘O) con miniaturas, teclas y botones, o el comando `pages 3,1,2r`; guarda una copia', 'Desde Visualización > Miniaturas o Hoja de contactos arrastras páginas o pulsas Suprimir'],
      ['Comprimir', 'Necesita Ghostscript (`brew install ghostscript`); tres ajustes (Light, Balanced, Strong); guarda «… compressed.pdf»', 'Archivo > Exportar con el filtro Quartz «Reducción del tamaño de archivo»; Apple avisa de que la calidad puede bajar'],
      ['Qué pasa con el original', 'Queda intacto: siempre se escribe una copia y nunca se sobrescribe', 'Apple avisa de que los cambios se guardan automáticamente y recomienda Archivo > Duplicar si quieres conservar los originales'],
      ['PDF con contraseña', 'El organizador lo rechaza («PDF is locked»); no hay paso para desbloquearlo', 'No se describe en las páginas revisadas'],
    ],
    note: `Vista Previa: [Combinar archivos PDF](${COMBINE}), [Añadir, eliminar o mover páginas de un PDF](${MOVE}) y [Reducir el tamaño de un PDF](${REDUCE}), consultadas el 28 de octubre de 2026. Ferrocast: documentación de [Merge PDFs](/docs/files/tools/merge-pdf/), [Split PDF](/docs/files/tools/split-pdf/) y [Organize PDF](/docs/files/tools/organize-pdf/) (en inglés), y el código de la app.`,
  },
  steps: {
    h: 'Cómo unir PDF con Ferrocast',
    howToName: 'Unir varios PDF en Mac con Ferrocast sin subirlos',
    items: [
      ['Instala Ferrocast', 'Descarga el DMG o instálalo con Homebrew. Unir, separar y reordenar no necesitan nada más.'],
      ['Elige el orden', 'Ejecuta Convert Files desde el lanzador y elige los PDF. En la lista elige «Choose order…», ordena las filas arrastrando o con ⌘↑ y ⌘↓, y pulsa ↵ (Merge).'],
      ['O únelos al instante', 'Arrastra los PDF desde el Finder, toca ⌘ para abrir la rueda de herramientas y suéltalos sobre Merge PDF. Si el orden importa, usa el paso anterior.'],
      ['Abre el resultado', 'Ferrocast escribe «… merged.pdf» junto al primer archivo y deja los originales como estaban.'],
    ],
  },
  sections: [
    {
      h: 'Separar un PDF en páginas',
      paras: [
        'Selecciona un PDF, abre la rueda de herramientas (arrastra y toca ⌘) y suelta sobre Split PDF. Ferrocast escribe un archivo por página junto al original: «… page 1.pdf», «… page 2.pdf» y así hasta la última. Con varios PDF seleccionados trabaja con el primero.',
        'Para quedarte con unas páginas concretas, abre el PDF en el organizador (⇧⌘O), quita las que no quieres con ⌫ y pulsa ↵: guarda una copia solo con las que quedan. También sirve el comando `pages 3,4,5` con el PDF seleccionado, que acepta cada página por separado (no intervalos como `3-5`).',
      ],
    },
    {
      h: 'Reordenar páginas con miniaturas',
      paras: [
        'Pulsa ⇧⌘O sobre un PDF. Arrastra una miniatura o usa ⌘← y ⌘→ para mover la página seleccionada; R la gira, D la duplica y ⌫ la quita. Cada acción tiene también un botón. ↵ guarda una copia llamada «… organized» junto al original. En un PDF largo no se puede arrastrar más allá de la tira visible: usa ⌘← y ⌘→. Más detalles en [la página del organizador](/tools/organize-pdf/) (en inglés).',
      ],
    },
    {
      h: 'Comprimir un PDF',
      paras: [
        'Selecciona el PDF y elige Compress en la rueda de herramientas. Ferrocast lo ejecuta con Ghostscript, que tienes que instalar aparte (`brew install ghostscript`), y ofrece tres ajustes: Light, Balanced y Strong. Escribe «… compressed.pdf»; si el resultado no es más pequeño, lo conserva igualmente y te lo indica. Vista Previa también puede reducir el tamaño con un filtro Quartz.',
      ],
    },
    {
      h: 'Cuándo quedarte con Vista Previa',
      list: [
        'Ya estás dentro de Vista Previa y solo vas a mover un par de páginas.',
        'Quieres mover páginas de un PDF a otro arrastrándolas: Ferrocast no tiene esa acción.',
        'No quieres instalar nada y no necesitas el orden en una ventana aparte.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Cómo unir varios PDF en Mac sin subirlos?', 'Con Ferrocast, arrastra los PDF desde el Finder, toca ⌘ y suéltalos sobre Merge PDF; o ejecuta Convert Files, elige los PDF, abre «Choose order…», ordénalos y pulsa ↵. El archivo «… merged.pdf» se escribe en tu Mac. Vista Previa también puede, arrastrando miniaturas de un PDF a otro.'],
      ['¿Vista Previa ya une PDF?', 'Sí. La guía de Apple explica cómo añadir páginas de un PDF a otro o fusionar todo el contenido de dos o más PDF arrastrando miniaturas. Avisa de que los cambios se guardan automáticamente y recomienda duplicar los PDF antes si quieres conservar los originales.'],
      ['¿Puedo separar un PDF por intervalos de páginas?', 'Split PDF de Ferrocast crea un archivo por cada página, no por intervalos. Para quedarte con unas páginas, ábrelas en el organizador, quita las demás y guarda una copia.'],
      ['¿Cómo reordeno las páginas de un PDF?', 'Pulsa ⇧⌘O sobre el PDF, arrastra las miniaturas o usa ⌘← y ⌘→, y pulsa ↵. Ferrocast guarda una copia y no sobrescribe el original. También puedes escribir `pages 3,1,2r` en el lanzador.'],
      ['¿Se suben mis PDF a algún servidor?', 'No. Ferrocast trabaja en tu Mac: unir usa la herramienta de PDF del propio macOS, separar y reordenar usan PDFKit, y comprimir usa Ghostscript si lo instalas.'],
      ['¿Cómo comprimo un PDF en Mac?', 'En Ferrocast elige Compress con un PDF; necesita Ghostscript y ofrece tres ajustes. En Vista Previa, elige Archivo > Exportar y el filtro Quartz «Reducción del tamaño de archivo»; Apple avisa de que la calidad puede bajar.'],
      ['¿Sobrescribe el PDF original?', 'No. Ferrocast siempre escribe un archivo nuevo junto al original, con «merged», «page N», «organized» o «compressed» en el nombre.'],
      ['¿Funciona con PDF con contraseña?', 'El organizador rechaza un PDF bloqueado con el aviso «PDF is locked». Ferrocast no desbloquea PDF.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Organizador de PDF (en inglés)', '/tools/organize-pdf/'],
      ['Merge PDFs (en inglés)', '/tools/merge-pdf/'],
      ['Split PDF (en inglés)', '/tools/split-pdf/'],
      ['Convertir PDF a Word', '/es/convertir/pdf-a-word/'],
      ['Convertir archivos en Mac', '/es/convertir/'],
      ['Quitar metadatos de un PDF', '/es/quitar-metadatos-mac/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
