/*
 * Copy for /remove-metadata-mac/ (English) and /es/quitar-metadatos-mac/ (Spanish), board idea "Quitar
 * ubicación y metadatos de fotos, vídeos, audio y PDF en Mac antes de compartir". Ferrocast facts come from
 * the code: crates/core/src/convert.rs (strip_jpeg drops APP1-APP15 and comments and copies the image data
 * as is; strip_png drops eXIf, tEXt, iTXt, zTXt and tIME), crates/app/src/services.rs (ImageOp::StripMetadata
 * only for JPEG and PNG, "metadata removal supports JPEG and PNG; convert to one of those first"; output
 * "<name> clean.<ext>"), crates/core/src/metaform.rs (ffmpeg -map 0 -c copy -map_metadata -1 -map_chapters -1),
 * crates/macos/src/pdf.rs (PDFKit attributes Title, Author, Subject, Keywords, Creator, Producer; output
 * "<name> edited.pdf"), crates/app/src/draw.rs (window notes) and docs/files/tools/edit-metadata.mdx.
 * Apple facts were read on CHECKED on support.apple.com (Personal Safety guide, Photos and Preview guides).
 * What a source does not say is written as "not stated" (never "no"). Rules: never promise that every tag is
 * gone, say that HEIC, WebP and TIFF are not handled, no speed numbers, no rival speed or memory claims.
 */
import type { GuideCopy } from './compress-video';
import { languageFaqEs, languageFaqEn } from './interface-language';

export const CHECKED = '2026-10-28';

const SAFETY = 'https://support.apple.com/guide/personal-safety/manage-location-metadata-in-photos-ips0d7a5df82/web';
const SAFETY_ES = 'https://support.apple.com/es-es/guide/personal-safety/ips0d7a5df82/web';
const PHOTOS_EXPORT = 'https://support.apple.com/guide/photos/export-photos-videos-slideshows-and-memories-pht6e157c5f/mac';
const PHOTOS_EXPORT_ES = 'https://support.apple.com/es-es/guide/photos/pht6e157c5f/mac';
const PV_LOCATION = 'https://support.apple.com/guide/preview/see-where-a-photo-was-taken-prvw19865/mac';
const PV_LOCATION_ES = 'https://support.apple.com/es-es/guide/preview/prvw19865/mac';
const PV_INFO = 'https://support.apple.com/guide/preview/view-information-about-pdfs-and-images-prvw9c94f0a4/mac';
const PV_INFO_ES = 'https://support.apple.com/es-es/guide/preview/prvw9c94f0a4/mac';

export const en: GuideCopy = {
  lang: 'en',
  path: '/remove-metadata-mac/',
  title: 'Remove EXIF and Location Data on Mac: Photo, Video, PDF | Ferrocast',
  description:
    'Strip GPS and other metadata from JPEG and PNG photos, video, audio, GIF and PDF on your Mac before sharing. Free, local, saves a copy. Limits listed.',
  crumbs: [['Home', '/'], ['Files', '/files/'], ['Remove metadata', '/remove-metadata-mac/']],
  h1: 'Remove location and metadata from photos, video, audio and PDF on a Mac',
  lead: [
    'Ferrocast strips metadata on your Mac and saves a copy next to the original. In a JPEG it drops the EXIF block (where GPS and camera tags live), XMP, IPTC and comments and copies the image data unchanged, so there is no recompression. In a PNG it drops the text and EXIF chunks. With ffmpeg installed it removes tags and chapters from video, audio and GIF without re-encoding, and it clears the document properties of a PDF. Nothing is uploaded.',
    'It is not a forensic scrubber. HEIC, WebP and TIFF photos are not handled directly, and “Remove all” leaves some container tags; the table and the limits below say what. Ferrocast is free and open source (MIT). I make it, so every Apple row links to Apple’s own page.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · ffmpeg only for video, audio and GIF' },
  limits: {
    h: 'Ferrocast, Photos and Preview for removing metadata',
    caption: 'Removing location and metadata on a Mac with Ferrocast, Photos and Preview compared',
    cols: ['', 'Ferrocast', 'Photos (Mac)', 'Preview (Mac)'],
    rows: [
      ['Location of a photo', 'Removes the whole EXIF block of a JPEG (GPS included) and the EXIF chunk of a PNG, into a copy', 'Select the photos, then Image > Location > Hide Location; Revert to Original Location undoes it', 'Apple’s page describes viewing the location (Tools > Show Location Info); removing it is not described there'],
      ['Other photo tags (camera, XMP, IPTC)', 'Dropped from a JPEG; text and EXIF chunks dropped from a PNG', 'Not stated in the pages checked', 'The Inspector shows image information; editing it is not described on that page'],
      ['Video', 'Yes, with ffmpeg: tags and chapters removed, streams copied', 'On export you choose whether metadata such as a title or caption is included; location is not mentioned on that page', 'Not covered in the pages checked'],
      ['Audio and GIF', 'Yes, with ffmpeg; a GIF can only be stripped, not edited', 'Not covered in the pages checked', 'Not covered in the pages checked'],
      ['PDF', 'Clears Title, Author, Subject and Keywords; Creator and Producer are shown but not written back', 'Not covered in the pages checked', 'The Inspector shows general information such as the author name; editing it is not described on that page'],
      ['HEIC, WebP, TIFF photos', 'Not handled: Ferrocast asks you to convert to JPEG or PNG first', 'Not stated in the pages checked', 'Not stated in the pages checked'],
      ['What happens to the original', 'Untouched; the copy is named “… clean” (photos) or “… edited” (everything else)', 'Not stated whether a copy is written; Revert to Original Location is offered', 'Apple says changes made to a Preview document are saved automatically'],
    ],
    note: `Apple: [Manage location metadata in Photos](${SAFETY}), [Export photos and videos in Photos](${PHOTOS_EXPORT}), [See where a photo was taken in Preview](${PV_LOCATION}) and [View information about PDFs and images](${PV_INFO}), read on 28 October 2026. Ferrocast: [Edit metadata docs](/docs/files/tools/edit-metadata/) and the app’s source code.`,
  },
  steps: {
    h: 'How to remove metadata with Ferrocast',
    howToName: 'Remove location and metadata from a file on a Mac with Ferrocast',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew. For video, audio and GIF also run brew install ffmpeg; photos and PDFs need nothing else.'],
      ['Open the Tools wheel', 'Drag the file from Finder and tap ⌘ while you hold it. You can drag several photos at once.'],
      ['Choose Metadata', 'For JPEG and PNG photos it runs at once and writes a copy named “… clean” next to each original.'],
      ['For video, audio, GIF and PDF, switch on Remove all metadata', 'A window lists the tags grouped by file, track and chapter. Turn on Remove all metadata and press ↵ (Save copy); the copy is named “… edited”.'],
      ['Check the copy', 'In Preview, open the photo and choose Tools > Show Location Info: if it is grayed out, the image has no location. For other files, open the copy in Ferrocast’s Edit metadata window to see what is left.'],
    ],
  },
  sections: [
    {
      h: 'What stays behind',
      list: [
        'In video, audio and GIF, “Remove all” leaves some tags that the format writes itself (the window says: “Container brands and some muxer tags stay”).',
        'Ferrocast copies every stream. I have not tested embedded cover art in audio files, which is stored as a stream, so do not assume it is removed.',
        'In a PDF, the window shows Creator and Producer but does not write them back. I have not checked whether other metadata stored in a PDF, such as an XMP packet, survives, so open the copy and look.',
        'In a PNG only the text and EXIF chunks (`eXIf`, `tEXt`, `iTXt`, `zTXt`, `tIME`) are removed; other chunks stay.',
        'A JPEG’s colour profile goes with the rest of the metadata, and so does the EXIF rotation flag. I have not tested whether either changes how a photo looks in other apps, so view the copy before you send it.',
        'Metadata is not content: faces, street signs and file names are not touched.',
      ],
    },
    {
      h: 'iPhone photos are HEIC: convert first',
      paras: [
        'Remove metadata runs on JPEG and PNG. On a HEIC, WebP or TIFF photo Ferrocast says “metadata removal supports JPEG and PNG; convert to one of those first”. Convert it with [HEIC to JPG](/convert/heic-to-jpg/), then run Remove metadata on the JPG. I have not checked whether a conversion carries the location across, which is why the second step matters: look at the result in Preview.',
      ],
    },
    {
      h: 'Another route: the Convert view',
      paras: [
        'You can also press ⌘T on a file result and run Convert Files. Photos show a “Remove metadata” action; a single video, audio, GIF or PDF opens the metadata window. That window works on one file at a time.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['How do I remove the location from a photo on a Mac?', 'Two ways. In Photos, select the photos and choose Image > Location > Hide Location, as Apple’s Personal Safety guide describes. Or drag a JPEG or PNG onto Ferrocast’s Tools wheel (tap ⌘) and choose Metadata: it writes a copy with the EXIF removed, GPS included, and leaves the original alone.'],
      ['Does removing metadata reduce the quality of a JPEG?', 'Not through Ferrocast. For a JPEG it removes the metadata segments and copies the image data unchanged, so there is no recompression. A PNG loses only its text and EXIF chunks.'],
      ['Can I remove metadata from a video on a Mac?', 'Yes, with ffmpeg installed. Ferrocast copies the streams, removes the tags and chapters and saves a copy named “… edited”. Some container tags that the format writes itself stay.'],
      ['How do I remove metadata from a PDF on a Mac?', 'Select the PDF, open Edit metadata from the Tools wheel, turn on Remove all metadata and press ↵. Ferrocast clears Title, Author, Subject and Keywords and saves a copy. Creator and Producer are shown but not written back.'],
      ['Does it work on HEIC photos from an iPhone?', 'Not directly: it handles JPEG and PNG. Convert the HEIC to JPG first, then remove the metadata from the JPG and check the result.'],
      ['Does “Remove all” remove everything?', 'No. Some container tags stay in video, audio and GIF, Creator and Producer are not written back in a PDF, and a PNG keeps chunks other than its text and EXIF ones. Open the copy and check before you share it.'],
      ['Can I share a photo from an iPhone without its location?', 'Apple’s Personal Safety guide says to select the photos in Photos, tap Share, tap Options and turn off Location, then share as usual.'],
      ['Is anything uploaded?', 'No. Ferrocast works on your Mac with its own code, ffmpeg and PDFKit.'],
    
      languageFaqEn,
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Edit metadata docs', '/docs/files/tools/edit-metadata/'],
      ['Edit metadata tool page', '/tools/edit-metadata/'],
      ['Spanish version', '/es/quitar-metadatos-mac/'],
      ['HEIC to JPG', '/convert/heic-to-jpg/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
      ['All file tools', '/tools/'],
      ['Key facts', '/facts/'],
    ],
  },
};

export const es: GuideCopy = {
  lang: 'es',
  path: '/es/quitar-metadatos-mac/',
  title: 'Quitar ubicación y metadatos en Mac: foto, vídeo y PDF | Ferrocast',
  description:
    'Quita el GPS y otros metadatos de fotos JPEG y PNG, vídeo, audio, GIF y PDF en tu Mac antes de compartir. Gratis, en local, guarda una copia. Con límites.',
  crumbs: [['Inicio', '/es/'], ['Archivos', '/es/files/'], ['Quitar metadatos', '/es/quitar-metadatos-mac/']],
  h1: 'Quitar la ubicación y los metadatos de fotos, vídeos, audio y PDF en Mac',
  lead: [
    'Ferrocast quita los metadatos en tu Mac y guarda una copia junto al original. En un JPEG elimina el bloque EXIF (donde viven el GPS y los datos de la cámara), XMP, IPTC y los comentarios, y copia los datos de la imagen sin tocarlos, así que no hay recompresión. En un PNG elimina los fragmentos de texto y EXIF. Con ffmpeg instalado quita etiquetas y capítulos de vídeo, audio y GIF sin recodificar, y vacía las propiedades de un PDF. No se sube nada. Puedes usarlo con la interfaz en español.',
    'No es un limpiador forense. Las fotos HEIC, WebP y TIFF no se tratan directamente y «Remove all» deja algunas etiquetas del contenedor; la tabla y los límites de abajo dicen cuáles. Ferrocast es gratis y de código abierto (MIT). Lo escribo yo, por eso cada fila de Apple enlaza a la página de Apple.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · ffmpeg solo para vídeo, audio y GIF' },
  limits: {
    h: 'Ferrocast, Fotos y Vista Previa para quitar metadatos',
    caption: 'Quitar ubicación y metadatos en Mac con Ferrocast, Fotos y Vista Previa',
    cols: ['', 'Ferrocast', 'Fotos (Mac)', 'Vista Previa (Mac)'],
    rows: [
      ['Ubicación de una foto', 'Quita todo el bloque EXIF de un JPEG (GPS incluido) y el fragmento EXIF de un PNG, en una copia', 'Selecciona las fotos y elige Imagen > Ubicación > Ocultar ubicación; «Revertir a ubicación original» lo deshace', 'La página de Apple describe cómo ver la ubicación (Herramientas > Mostrar información de ubicación); no describe cómo quitarla'],
      ['Otras etiquetas de una foto (cámara, XMP, IPTC)', 'Se eliminan de un JPEG; en un PNG se eliminan los fragmentos de texto y EXIF', 'No consta en las páginas revisadas', 'El Inspector muestra información de la imagen; editarla no se describe en esa página'],
      ['Vídeo', 'Sí, con ffmpeg: quita etiquetas y capítulos y copia los flujos', 'Al exportar eliges si se incluyen metadatos como el título o el subtítulo; esa página no menciona la ubicación', 'No se trata en las páginas revisadas'],
      ['Audio y GIF', 'Sí, con ffmpeg; un GIF solo se puede limpiar, no editar', 'No se trata en las páginas revisadas', 'No se trata en las páginas revisadas'],
      ['PDF', 'Vacía Title, Author, Subject y Keywords; Creator y Producer se muestran pero no se escriben de vuelta', 'No se trata en las páginas revisadas', 'El Inspector muestra información general, como el nombre del autor; editarla no se describe en esa página'],
      ['Fotos HEIC, WebP, TIFF', 'No se tratan: Ferrocast te pide convertirlas antes a JPEG o PNG', 'No consta en las páginas revisadas', 'No consta en las páginas revisadas'],
      ['Qué pasa con el original', 'Queda intacto; la copia se llama «… clean» (fotos) o «… edited» (el resto)', 'No consta si se escribe una copia; se ofrece «Revertir a ubicación original»', 'Apple indica que los cambios en un documento de Vista Previa se guardan automáticamente'],
    ],
    note: `Apple: [Eliminar metadatos de ubicación en Fotos](${SAFETY_ES}), [Exportar fotos y vídeos en Fotos](${PHOTOS_EXPORT_ES}), [Ver dónde se hizo una foto en Vista Previa](${PV_LOCATION_ES}) e [Información sobre PDF e imágenes](${PV_INFO_ES}), consultadas el 28 de octubre de 2026. Ferrocast: [documentación de Edit metadata](/docs/files/tools/edit-metadata/) (en inglés) y el código de la app.`,
  },
  steps: {
    h: 'Cómo quitar metadatos con Ferrocast',
    howToName: 'Quitar la ubicación y los metadatos de un archivo en Mac con Ferrocast',
    items: [
      ['Instala Ferrocast', 'Descarga el DMG o instálalo con Homebrew. Para vídeo, audio y GIF ejecuta también brew install ffmpeg; las fotos y los PDF no necesitan nada más.'],
      ['Abre la rueda de herramientas', 'Arrastra el archivo desde el Finder y toca ⌘ sin soltarlo. Puedes arrastrar varias fotos a la vez.'],
      ['Elige Metadata', 'En fotos JPEG y PNG se ejecuta al instante y escribe una copia llamada «… clean» junto a cada original.'],
      ['En vídeo, audio, GIF y PDF, activa Remove all metadata', 'Una ventana lista las etiquetas agrupadas por archivo, pista y capítulo. Activa Remove all metadata y pulsa ↵ (Save copy); la copia se llama «… edited».'],
      ['Comprueba la copia', 'En Vista Previa, abre la foto y elige Herramientas > Mostrar información de ubicación: si aparece en gris, la imagen no tiene ubicación. En otros archivos, abre la copia en la ventana Edit metadata de Ferrocast para ver lo que queda.'],
    ],
  },
  sections: [
    {
      h: 'Lo que se queda',
      list: [
        'En vídeo, audio y GIF, «Remove all» deja algunas etiquetas que escribe el propio formato (la ventana lo avisa: «Container brands and some muxer tags stay»).',
        'Ferrocast copia todos los flujos. No he probado las carátulas incrustadas en archivos de audio, que se guardan como un flujo, así que no des por hecho que se quitan.',
        'En un PDF, la ventana muestra Creator y Producer pero no los escribe de vuelta. No he comprobado si sobrevive otra información guardada en un PDF, como un paquete XMP, así que abre la copia y míralo.',
        'En un PNG solo se quitan los fragmentos de texto y EXIF (`eXIf`, `tEXt`, `iTXt`, `zTXt`, `tIME`); los demás se quedan.',
        'El perfil de color de un JPEG se va con el resto de metadatos, y también la marca de rotación de EXIF. No he probado si alguno de los dos cambia el aspecto de la foto en otras apps, así que mira la copia antes de enviarla.',
        'Los metadatos no son el contenido: las caras, los carteles de una calle y el nombre del archivo no se tocan.',
      ],
    },
    {
      h: 'Las fotos del iPhone son HEIC: conviértelas antes',
      paras: [
        'Remove metadata funciona con JPEG y PNG. En una foto HEIC, WebP o TIFF, Ferrocast avisa con «metadata removal supports JPEG and PNG; convert to one of those first». Conviértela con [HEIC a JPG](/es/convertir/heic-a-jpg/) y ejecuta Remove metadata sobre el JPG. No he comprobado si una conversión conserva la ubicación, y por eso importa el segundo paso: mira el resultado en Vista Previa.',
      ],
    },
    {
      h: 'Otra forma de llegar: la vista Convertir',
      paras: [
        'También puedes pulsar ⌘T sobre un resultado de archivo y ejecutar Convert Files. Las fotos muestran la acción «Remove metadata»; un vídeo, audio, GIF o PDF suelto abre la ventana de metadatos. Esa ventana trabaja con un archivo cada vez.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Cómo quito la ubicación de una foto en Mac?', 'De dos formas. En Fotos, selecciona las fotos y elige Imagen > Ubicación > Ocultar ubicación, como describe la guía de seguridad personal de Apple. O arrastra un JPEG o PNG a la rueda de herramientas de Ferrocast (toca ⌘) y elige Metadata: escribe una copia sin EXIF, GPS incluido, y deja el original como estaba.'],
      ['¿Quitar los metadatos de un JPEG reduce su calidad?', 'Con Ferrocast no. En un JPEG elimina los segmentos de metadatos y copia los datos de la imagen sin tocarlos, así que no hay recompresión. Un PNG solo pierde sus fragmentos de texto y EXIF.'],
      ['¿Puedo quitar los metadatos de un vídeo en Mac?', 'Sí, con ffmpeg instalado. Ferrocast copia los flujos, quita las etiquetas y los capítulos y guarda una copia llamada «… edited». Se quedan algunas etiquetas del contenedor que escribe el propio formato.'],
      ['¿Cómo quito los metadatos de un PDF en Mac?', 'Selecciona el PDF, abre Edit metadata desde la rueda de herramientas, activa Remove all metadata y pulsa ↵. Ferrocast vacía Title, Author, Subject y Keywords y guarda una copia. Creator y Producer se muestran pero no se escriben de vuelta.'],
      ['¿Funciona con fotos HEIC del iPhone?', 'No directamente: trata JPEG y PNG. Convierte antes el HEIC a JPG, quita los metadatos del JPG y comprueba el resultado.'],
      ['¿«Remove all» quita absolutamente todo?', 'No. En vídeo, audio y GIF se quedan algunas etiquetas del contenedor; en un PDF, Creator y Producer no se escriben de vuelta, y un PNG conserva los fragmentos que no son de texto ni EXIF. Abre la copia y compruébalo antes de compartirla.'],
      ['¿Puedo compartir una foto desde el iPhone sin su ubicación?', 'La guía de seguridad personal de Apple indica que selecciones las fotos en Fotos, toques Compartir, toques Opciones y desactives Ubicación, y que compartas como siempre.'],
      ['¿Se sube algo?', 'No. Ferrocast trabaja en tu Mac con su propio código, ffmpeg y PDFKit.'],
    
      languageFaqEs,
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Documentación de Edit metadata (en inglés)', '/docs/files/tools/edit-metadata/'],
      ['Página de la herramienta Edit metadata (en inglés)', '/tools/edit-metadata/'],
      ['English version', '/remove-metadata-mac/'],
      ['Convertir archivos en Mac', '/es/convertir/'],
      ['Convertidor de archivos gratis para Mac', '/es/convertidor-de-archivos-gratis-mac/'],
      ['Herramientas de archivo', '/es/tools/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
