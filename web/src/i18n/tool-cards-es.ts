/** Spanish chrome for the file-tool cards. Tool names and typed commands stay the app’s English UI. */
export const toolCardGroupEs: Record<string, { name: string; blurb: string }> = {
  image: { name: 'Editores de imagen', blurb: 'Para JPEG, PNG, WebP, HEIC y TIFF. Los originales nunca se sobrescriben.' },
  video: { name: 'Ventanas de vídeo', blurb: 'Para MP4, MOV y MKV. Los fotogramas de vista previa y la recodificación necesitan ffmpeg.' },
  pdf: { name: 'Organizador de PDF y metadatos', blurb: 'Reordena, divide y une PDF, y lee o cambia etiquetas de los archivos.' },
  audio: { name: 'Herramientas de audio', blurb: 'Para MP3, M4A, WAV y FLAC. Usan ffmpeg.' },
  archive: { name: 'Archivos comprimidos y compresión', blurb: 'Reduce el tamaño, o descomprime un archivo en una carpeta nueva junto a él.' },
};

/**
 * One line per tool. `redactVideo` follows the docs (several areas, each with its own rectangle),
 * not the English card’s “One area per run”.
 */
export const toolCardAboutEs: Record<string, string> = {
  editImage: 'Controles de brillo, contraste, saturación, calidez, claridad, enfoque, grano, reducción de ruido y quitar velo, con vista previa sobre una copia más pequeña.',
  annotateImage: 'Flechas, líneas, cajas, elipses, resaltados, lápiz, etiquetas numeradas, lupas, texto, llamadas y sellos de imagen. Guarda una copia PNG.',
  frameImage: 'Pon la imagen sobre un fondo de color o desenfocado, con margen, esquinas redondeadas y sombra.',
  cropImage: 'Arrastra un rectángulo, fija una proporción o escribe un ancho y un alto exactos.',
  redactImage: 'Arrastra tus propias zonas sobre la imagen, o escribe redact faces o redact text para detectarlas en tu Mac. Color sólido o desenfoque.',
  createCollage: 'Diseños de cuadrícula, fila, columna o destacado, con separación, esquinas redondeadas y un fondo.',
  createPdf: 'Junta las imágenes seleccionadas en un PDF, una página por imagen, en el orden que elijas.',
  trimVideo: 'Avanza por fotograma o por segundo, marca el inicio y el fin, y escribe un corte recodificado exacto.',
  cropVideo: 'Arrastra un rectángulo sobre un fotograma de vista previa o escribe las medidas exactas.',
  changeVideoSpeed: 'Desde un cuarto de la velocidad hasta cuatro veces, con la nueva duración a la vista. Sin vista previa de reproducción.',
  videoSnapshots: 'Elige fotogramas y guárdalos como PNG a resolución completa.',
  splitVideo: 'Añade puntos de corte en la línea de tiempo o divide en partes iguales. Los clips van a una carpeta nueva.',
  redactVideo: 'Tapa varias zonas con un color sólido o un desenfoque, cada una con su propio intervalo. Cada zona tiene su rectángulo.',
  joinVideos: 'Une varios clips en un MP4 en el orden que elijas; el primer clip fija el lienzo.',
  muteVideo: 'Escribe una copia del vídeo sin su pista de audio.',
  organizePdf: 'Miniaturas de página: gira, duplica, quita y reordena las páginas, y luego guarda una copia.',
  mergePdf: 'Une PDF en el orden que elijas.',
  splitPdf: 'Divide un PDF en un archivo por página.',
  removeMetadata: 'Lee etiquetas de audio, vídeo y PDF, edítalas o quítalas. Las imágenes usan un ajuste de borrado sin pérdida.',
  normalizeAudio: 'Fija una sonoridad objetivo, un rango de sonoridad y un pico real, y mira primero el nivel medido.',
  trimAudio: 'Forma de onda con marcas de inicio y fin, tiempos exactos y un botón para cortar el silencio de los extremos.',
  audioChannels: 'Mono o estéreo, con el volumen izquierdo y derecho en dB.',
  audioToVideo: 'Crea un MP4 a partir de audio con una imagen fija. La imagen fija se indica con una orden escrita.',
  redactAudio: 'Tapa un tramo de audio con un pitido mediante una orden escrita.',
  compress: 'Apunta a un tamaño, un porcentaje o un ajuste, y opcionalmente limita el lado más largo. Comprimir PDF necesita Ghostscript.',
  extractArchive: 'Descomprime ZIP, TAR, GZ o RAR en una carpeta nueva con el nombre del archivo. Rechaza entradas que intentan salir de esa carpeta.',
};

export const toolCardUiEs = {
  type: 'Escribe',
  shortcut: 'sobre un archivo seleccionado en la lista del lanzador',
  byFormat: 'Qué herramientas aparecen para cada formato',
  byFormatLead: 'La rueda y la lista Convert ofrecen estas herramientas. «Varios archivos» es lo que aparece al seleccionar varios.',
  tableAria: 'Herramientas por formato, con desplazamiento',
  tableCaption: 'Herramientas que se ofrecen para cada formato',
  format: 'Formato',
  oneFile: 'Un archivo',
  several: 'Varios archivos',
};
