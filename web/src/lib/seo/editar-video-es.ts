/*
 * Copy for /es/editar-video-mac-sin-imovie/ (Spanish only, board idea "Recortar, dividir, unir, acelerar y
 * quitar el sonido a un vídeo en Mac sin iMovie y sin subirlo"). Ferrocast facts come from
 * crates/core/src/media.rs (ffmpeg arguments: trim, crop, speed 0.25..=4 with pitch kept, join, mute),
 * crates/core/src/convert.rs (output suffixes, typed commands), crates/app/src/services.rs (split every N
 * seconds as copied segments) and docs/files/tools/{trim,split,join,speed,mute,crop}-video. Nothing is
 * claimed about iMovie or any other editor. Rules: say that ffmpeg is required, with the brew command;
 * never promise the same quality after a re-encode; neutral Spanish from Spain, tú.
 */
import type { GuideCopy } from './compress-video';

export const page: GuideCopy = {
  lang: 'es',
  path: '/es/editar-video-mac-sin-imovie/',
  title: 'Editar vídeo en Mac sin iMovie: recortar, unir, acelerar | Ferrocast',
  description:
    'Recorta, divide, une, acelera, recorta el encuadre y quita el sonido a un vídeo en tu Mac sin iMovie y sin subirlo. Gratis, local, necesita ffmpeg.',
  crumbs: [['Inicio', '/es/'], ['Editar vídeo sin iMovie', '/es/editar-video-mac-sin-imovie/']],
  h1: 'Editar un vídeo en Mac sin iMovie y sin subirlo a ningún sitio',
  lead: [
    'Ferrocast hace seis ediciones rápidas de vídeo en tu Mac, desde el mismo menú de herramientas: recortar, dividir, unir, cambiar la velocidad (de 0,25× a 4×, manteniendo el tono), recortar el encuadre y quitar el sonido. Todo se hace en tu Mac con ffmpeg y no se sube nada.',
    'Es para ediciones sueltas, no para montar una película: no hay línea de tiempo con varias pistas ni transiciones. Necesita ffmpeg (`brew install ffmpeg`). Escribo Ferrocast, así que esta página solo describe lo que hace Ferrocast.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · necesita ffmpeg' },
  limits: {
    h: 'Las seis ediciones de un vistazo',
    caption: 'Herramientas de vídeo de Ferrocast: cómo se abren y qué hacen con el archivo',
    cols: ['Tarea', 'Cómo se abre', 'Qué hace con el archivo'],
    rows: [
      ['Recortar', '⇧⌘V sobre un vídeo, o `trim 0:10-0:30`', 'Con el visor, un corte exacto al fotograma (se recodifica); con el comando escrito, copia sin recodificar y el corte cae en un fotograma clave'],
      ['Dividir', 'Herramienta Split, o `split 30s`', 'Un clip por cada marcador o en partes iguales (de 2 a 50), todos en una carpeta nueva; el original no se toca'],
      ['Unir', 'Selecciona varios vídeos y elige Join', 'Un solo MP4: el primer clip fija el lienzo y los clips sin sonido reciben silencio generado'],
      ['Velocidad', '⇧⌘L sobre un vídeo, o `speed 1.5x`', 'De 0,25× a 4× manteniendo el tono, con la duración resultante calculada; se recodifica en H.264'],
      ['Recortar el encuadre', '⇧⌘J sobre un vídeo, o `crop 640x360 at 100,50`', 'Un rectángulo que arrastras, proporciones predefinidas o ancho y alto exactos; se conserva el audio'],
      ['Quitar el sonido', 'Herramienta Mute video, o `mute`', 'Una copia sin sonido; la imagen se copia sin recodificar'],
    ],
    note: 'Fuente: la [documentación de herramientas de archivo](/docs/files/tools/trim-video/) (en inglés) y el código de la app. Cada edición escribe una copia nueva junto al original, con un sufijo en el nombre (trimmed, part, muted, speed, cropped, joined), y no sobrescribe el original.',
  },
  steps: {
    h: 'Cómo recortar un vídeo en Mac',
    howToName: 'Recortar un vídeo en Mac sin iMovie con Ferrocast',
    items: [
      ['Instala ffmpeg una vez', 'Ejecuta `brew install ffmpeg` en el Terminal. Sin ffmpeg estas herramientas no funcionan. Está en [Optional tools](/docs/files/optional-tools/) (en inglés).'],
      ['Abre el vídeo en Ferrocast', 'Arrastra el vídeo y toca ⌘ para abrir la rueda de herramientas, o elígelo en la vista Convert.'],
      ['Elige Trim y marca el principio y el final', 'Pulsa ⇧⌘V. Las flechas avanzan un fotograma, ⇧ con las flechas avanza un segundo, I y O fijan el inicio y el final. También puedes usar los botones o hacer clic en la barra.'],
      ['Guarda la copia', 'Pulsa ↵. Ferrocast escribe el recorte exacto como una copia nueva con «trimmed» en el nombre, junto al original.'],
    ],
  },
  sections: [
    {
      h: 'Recortar y dividir',
      paras: [
        'El visor de recorte muestra el fotograma del cursor, una línea de tiempo y los marcadores de inicio y fin. ↵ escribe un corte exacto al fotograma, y para eso recodifica el vídeo. El comando escrito `trim 0:10-0:30` copia sin recodificar, pero el corte cae en un fotograma clave.',
        'Dividir corta un vídeo en varios clips: M añade un corte en el cursor y ⌫ quita el último; si no hay marcadores, indicas partes iguales (de 2 a 50). `split 30s` corta cada 30 segundos copiando sin recodificar. Una división que produciría menos de dos clips se rechaza con un mensaje.',
      ],
    },
    {
      h: 'Unir clips',
      paras: [
        'Selecciona varios vídeos y elige Join. Una ventana te deja ordenar los clips con ⌘↑ y ⌘↓ o arrastrando las filas. El resultado es un MP4 (H.264 y AAC): el primer clip fija el tamaño y los demás se ajustan a él con bandas si hace falta.',
      ],
    },
    {
      h: 'Acelerar o ralentizar',
      paras: [
        'La ventana de velocidad ofrece un control de 0,25× a 4× y valores predefinidos, y calcula la duración del resultado. Mantiene el tono del audio. Ferrocast no reproduce el vídeo en esa ventana. Con el comando escrito: `speed 1.5x`.',
      ],
    },
    {
      h: 'Recortar el encuadre y quitar el sonido',
      paras: [
        'Crop muestra un fotograma real con un rectángulo que arrastras, proporciones predefinidas o ancho y alto exactos, y mantiene el audio. El fotograma de referencia está fijo al 10 % de la duración. Mute hace una copia sin sonido y no recodifica la imagen.',
      ],
    },
    {
      h: 'Límites',
      list: [
        'Necesita ffmpeg (`brew install ffmpeg`).',
        'Los recortes, el cambio de velocidad y el encuadre recodifican la imagen: no prometo la misma calidad que el original.',
        'Los vídeos WMV fallan al quitar el sonido y al recortar el encuadre.',
        'No hay línea de tiempo con varias pistas, transiciones, títulos ni música: es para ediciones sueltas.',
        'Solo Mac con Apple Silicon.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Puedo recortar un vídeo en Mac sin iMovie?', 'Sí. En Ferrocast, que es gratis y de código abierto, pulsas ⇧⌘V sobre el vídeo, marcas el inicio y el final y pulsas ↵. Necesita ffmpeg y no sube el vídeo a ningún sitio.'],
      ['¿Hay que subir el vídeo a una web?', 'No. Las seis ediciones se hacen en tu Mac con ffmpeg y escriben una copia junto al original.'],
      ['¿Se pierde calidad al recortar?', 'El visor hace un corte exacto al fotograma y recodifica el vídeo, así que no se garantiza la misma calidad. El comando escrito `trim` copia sin recodificar, pero el corte cae en un fotograma clave.'],
      ['¿Cómo acelero un vídeo sin que cambie el tono?', 'Pulsa ⇧⌘L sobre el vídeo y elige una velocidad entre 0,25× y 4×. Ferrocast mantiene el tono del audio y muestra la duración resultante. También puedes escribir `speed 1.5x`.'],
      ['¿Cómo quito el sonido a un vídeo?', 'Elige Mute video en la rueda de herramientas o escribe `mute`. Ferrocast crea una copia sin sonido y no toca el original.'],
      ['¿Puedo unir varios vídeos en uno?', 'Sí. Selecciona los vídeos, elige Join y ordénalos. El resultado es un MP4 y el primer clip fija el tamaño del lienzo.'],
      ['¿Qué necesito instalar?', 'Ferrocast y ffmpeg, que se instala con `brew install ffmpeg`. Funciona solo en Mac con Apple Silicon.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Comprimir un vídeo a un tamaño', '/es/comprimir-video-a-un-tamano/'],
      ['Difuminar parte de un vídeo', '/es/difuminar-video-mac/'],
      ['Convertir un vídeo a GIF', '/es/convertir/video-a-gif/'],
      ['Recortar vídeo (en inglés)', '/tools/trim-video/'],
      ['Acelerar vídeo (en inglés)', '/tools/video-speed/'],
      ['Herramientas de archivo', '/es/tools/'],
    ],
  },
};
