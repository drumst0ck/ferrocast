import { counts } from '../data/status';

/** Spanish lines for /es/tools/. Each one translates the English tool doc description. */
export const toolDescriptionsEs: Record<string, string> = {
  'add-background': 'Coloca una imagen sobre un color, una copia desenfocada u otra imagen en Ferrocast, con margen, esquinas redondeadas y sombra. La vista previa es en directo y se guarda una copia.',
  annotate: 'Anota una imagen en Ferrocast con flechas, cajas, resaltados, etiquetas numeradas, texto, llamadas y una lupa en su propio lienzo. Guarda una copia PNG.',
  'audio-channels': 'Pasa un audio de mono a estéreo o al revés y ajusta el volumen izquierdo y derecho en dB con Ferrocast, en una copia nueva junto al original. Necesita ffmpeg.',
  'audio-to-video': 'Crea un MP4 a partir de un audio con Ferrocast, con la forma de onda o una imagen fija en horizontal, vertical o cuadrado. Necesita ffmpeg.',
  'bleep-audio': 'Sustituye varios tramos de un audio por un tono de 1 kHz en Ferrocast, elegidos sobre la forma de onda. Solo para archivos de audio. Necesita ffmpeg.',
  collage: 'Junta dos o más imágenes en un collage en Ferrocast: cuadrícula, fila, columna o destacado, con ancho, separación, esquinas y fondo.',
  compress: 'Comprime imágenes por tamaño, porcentaje o ajuste, y archivos de audio, vídeo y PDF, con Ferrocast. El resultado es una copia nueva junto al original.',
  'create-pdf': 'Junta las imágenes seleccionadas en un PDF con Ferrocast, una página por imagen, en el orden que elijas. El resultado queda junto a los originales.',
  'crop-image': 'Recorta una imagen en Ferrocast con un rectángulo que arrastras, proporciones fijas como 1:1 o 16:9, o un ancho y un alto exactos. Guarda una copia.',
  'crop-video': 'Recorta el fotograma de un vídeo en Ferrocast con un rectángulo que arrastras, proporciones predefinidas o un ancho y un alto exactos. Necesita ffmpeg.',
  'edit-metadata': 'Consulta, edita o quita metadatos de audio, vídeo, GIF y PDF en Ferrocast, agrupados por archivo, pista y capítulo. Las imágenes usan órdenes escritas.',
  'edit-photos': 'Edita fotos en Ferrocast con vista previa en directo y nueve controles: brillo, contraste, saturación, calidez, claridad, enfoque, grano, reducción de ruido y quitar velo.',
  'extract-archive': 'Extrae archivos zip, tar, tar.gz, gz y rar a una carpeta nueva junto al archivo con Ferrocast, tras comprobar que los nombres de las entradas no salen de esa carpeta.',
  'join-videos': 'Une varios clips de vídeo en un MP4 con Ferrocast, eligiendo el orden. Los clips sin audio reciben silencio generado. Necesita ffmpeg.',
  'merge-pdf': 'Une dos o más PDF en uno con Ferrocast, eligiendo el orden en la ventana Choose Order. El resultado es una copia nueva junto a los originales.',
  'mute-video': 'Quita el sonido de un vídeo con Ferrocast en un paso, en una copia nueva junto al original, de modo que el archivo de origen no cambia. Necesita ffmpeg.',
  'normalize-audio': 'Ajusta la sonoridad de un audio con Ferrocast: LUFS objetivo, rango de sonoridad y pico real, medidos antes. Necesita ffmpeg.',
  'organize-pdf': 'Reordena, gira, duplica y quita páginas de un PDF con miniaturas en Ferrocast, o escribe un orden de páginas. Guarda una copia y nunca sobrescribe.',
  'redact-image': 'Redacta caras o texto en tu Mac con Vision, o tapa zonas que arrastras con un color sólido o un desenfoque. Ferrocast guarda el resultado como copia.',
  'redact-video': 'Tapa varias zonas de un vídeo durante los intervalos que elijas, con un color sólido o un desenfoque, en Ferrocast. Cada zona tiene su rectángulo. Necesita ffmpeg.',
  'split-pdf': 'Divide un PDF en un archivo por página con Ferrocast desde la vista Convert. El resultado se escribe junto al original y nunca lo sobrescribe.',
  'split-video': 'Divide un vídeo en clips con marcas de corte o partes iguales en Ferrocast. Todos los clips van a una carpeta nueva. Necesita ffmpeg.',
  'trim-audio': 'Corta audio en Ferrocast sobre una forma de onda, con casillas de inicio y fin exactos y el botón Trim silence, en una copia nueva junto al original. Necesita ffmpeg.',
  'trim-video': 'Corta un vídeo en Ferrocast con un visor preciso al fotograma, una línea de tiempo y marcas de inicio y fin, o un rango escrito. Necesita ffmpeg.',
  'video-snapshots': 'Guarda un PNG a resolución completa de cualquier fotograma con Ferrocast, eligiendo el fotograma en una línea de tiempo o avanzando fotograma a fotograma. Necesita ffmpeg.',
  'video-speed': 'Acelera o ralentiza un vídeo en Ferrocast entre 0,25× y 4×, con ajustes predefinidos y la duración calculada, manteniendo el tono. Necesita ffmpeg.',
};

export const toolGroupLabelsEs: Record<string, string> = {
  image: 'Herramientas de imagen',
  video: 'Herramientas de vídeo',
  audio: 'Herramientas de audio',
  pdf: 'Herramientas de PDF',
  archive: 'Archivos comprimidos',
  any: 'Cualquier tipo de archivo',
};

export function toolsEsMeta(toolCount = counts().tools): { title: string; description: string } {
  const title = 'Herramientas de archivo para Mac · Ferrocast';
  const description = `${toolCount} herramientas locales para imagen, vídeo, audio y PDF: comprimir, recortar, cortar, dividir, unir, tapar y más. No se sube nada.`;
  if (title.length > 60 || description.length < 120 || description.length > 155) {
    throw new Error(`tools es meta out of range: ${title.length}/${description.length}`);
  }
  return { title, description };
}
