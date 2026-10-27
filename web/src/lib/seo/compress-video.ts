/*
 * Copy for the "compress a video to a target size" task pages (/compress-video-to-size/ and
 * /es/comprimir-video-a-un-tamano/). Source: the Notion brief "Brief: Comprimir vídeo a un tamaño".
 * Platform limits come from official pages (see `limits`), checked on CHECKED. Ferrocast facts come from
 * crates/core/src/compress.rs (bitrate planning), crates/core/src/media.rs (compress_video: one-pass
 * libx264 + AAC MP4) and the Compress window in crates/app/src/draw.rs.
 * Rules from the brief: never promise an "exact size" or "no quality loss"; no speed or quality
 * comparisons with other apps; no platform limit without an official source.
 */

export const CHECKED = '2026-10-27';

/** Measured with Ferrocast's own ffmpeg arguments on a noisy 60 s 1080p60 test clip (see the feature log). */
export const MEASURED = { tenMiB: 10.07, twentyMiB: 20.07 };

export interface GuideCopy {
  lang: 'en' | 'es';
  path: string;
  title: string;
  description: string;
  crumbs: [string, string][];
  h1: string;
  lead: string[];
  cta: { dmg: string; note: string };
  /** The page's table: any number of columns, the first cell of each row is its header. */
  limits: { h: string; caption: string; cols: string[]; rows: string[][]; note: string };
  steps: { h: string; howToName: string; items: [string, string][]; code?: string };
  sections: { h: string; paras: string[]; list?: string[] }[];
  faq: { h: string; items: [string, string][] };
  more: { h: string; links: [string, string][] };
}

export const en: GuideCopy = {
  lang: 'en',
  path: '/compress-video-to-size/',
  title: 'Compress a video to a target size on Mac (Discord, email) | Ferrocast',
  description:
    'Shrink a video to fit Discord’s 20 MiB or Gmail’s 25 MB on your Mac, without uploading it. Ferrocast works out the bitrate from the size you type. Free.',
  crumbs: [['Home', '/'], ['Files', '/files/'], ['Compress a video to a size', '/compress-video-to-size/']],
  h1: 'Compress a video to a target size on your Mac',
  lead: [
    'Type the size you need and Ferrocast works out the bitrate from the video’s length, then writes a smaller MP4 next to the original. Nothing is uploaded. Ferrocast is free and open source (MIT) and uses ffmpeg on your Mac for video.',
    'It aims for the size; it does not guarantee it to the byte. In our test the result came out up to 0.7 % above the target, so leave a small margin under the limit.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · needs ffmpeg for video' },
  limits: {
    h: 'What size to type',
    caption: 'Upload limits and the target size to type in Ferrocast',
    cols: ['Where it goes', 'Limit', 'Type in Ferrocast'],
    rows: [
      ['Discord, without Nitro', '20 MiB per file (since 3 September 2026; it was 10 MiB)', '`19000 KB`'],
      ['Gmail attachment', '25 MB in total; above that Gmail adds a Google Drive link', '`23000 KB`'],
    ],
    note: 'Sources: [Discord developer reference](https://github.com/discord/discord-api-docs/blob/main/developers/reference.mdx) and [Gmail Help](https://support.google.com/mail/answer/6584), checked on 27 October 2026. Ferrocast’s size field is in KB (1 KB = 1,024 bytes), so 19000 KB is about 18.6 MiB.',
  },
  steps: {
    h: 'How to do it',
    howToName: 'Compress a video to a target size on a Mac with Ferrocast',
    items: [
      ['Install Ferrocast and ffmpeg', 'Install Ferrocast (DMG or Homebrew) and ffmpeg with brew install ffmpeg. Video compression needs ffmpeg.'],
      ['Select the video', 'Select the video in the launcher, or drag it from Finder and tap ⌘ for the Tools wheel. You can select several videos.'],
      ['Open Compress…', 'Choose Compress… and set the goal to Size.'],
      ['Type the target size', 'Type the size in KB, for example 19000 for Discord. The window shows the bitrates it will use, and refuses a target that is too small for the video’s length.'],
      ['Resize if the video is long', 'Optionally turn on Resize and set the longest edge (for example 1280). It never upscales.'],
      ['Press Enter and check the size', 'The copy is written next to the original as “name compressed.mp4” and never overwrites it. Check its size in Finder before you upload.'],
    ],
    code: 'brew install ffmpeg',
  },
  sections: [
    {
      h: 'How the size becomes a bitrate',
      paras: [
        'A video’s size is roughly its bitrate times its length. Ferrocast divides the target size by the duration, keeps a share for audio (128 kbit/s, or 96, 64 or 32 when the budget is tight) and gives the rest to the picture. The video is re-encoded once as H.264 with AAC audio in an MP4 file.',
        'Example: a 60-second clip at 19000 KB gets about 2.5 Mbit/s in total, which is plenty for a 1080p screen recording and tight for a fast game clip.',
      ],
    },
    {
      h: 'When to lower the resolution',
      paras: ['The longer the video, the fewer bits per second fit in the same size. For long clips, lowering the resolution often looks better than keeping 1080p at a very low bitrate.'],
      list: ['Under a minute for Discord: usually fine at the original size.', 'A few minutes: try Resize at 1280.', 'Much longer: 720 or 480, or split the video first with Split video.'],
    },
    {
      h: 'Limits',
      paras: [],
      list: [
        'It aims for the size in one pass, so the result can land slightly above it (up to 0.7 % in our test). Keep about 5 % under the limit.',
        'Video compression needs ffmpeg; Ferrocast tells you when it is missing.',
        'The output is always an H.264 MP4.',
        'Typing “25mb” in the Convert view works for images only; for video use the Compress… window.',
        'Apple Silicon Macs only.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['What size should I use for Discord?', 'Without Nitro, Discord accepts files up to 20 MiB since 3 September 2026. In Ferrocast type 19000 KB to leave a margin.'],
      ['Is my video uploaded anywhere?', 'No. Ferrocast compresses the video on your Mac with ffmpeg; nothing is uploaded.'],
      ['Will it hit the exact size?', 'Not to the byte. Ferrocast computes the bitrate from the size and length and encodes once, so the result lands near the target and can be slightly above it. Leave about 5 % of margin.'],
      ['Do I need ffmpeg?', 'Yes, for video. Install it with brew install ffmpeg. Images can be compressed without it.'],
      ['Does it overwrite my original?', 'No. The smaller copy is saved next to the original as “name compressed.mp4”.'],
      ['Can I compress several videos at once?', 'Yes. Select several videos and open Compress…; each one is compressed to the same target size.'],
      ['Is Ferrocast free?', 'Yes. Ferrocast is free and open source under the MIT licence, with no account and no paid plan.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Compress tool', '/tools/compress/'],
      ['Split video', '/tools/split-video/'],
      ['Trim video', '/tools/trim-video/'],
      ['Video conversions', '/convert/video/'],
      ['Optional tools (ffmpeg)', '/docs/files/optional-tools/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
    ],
  },
};

export const es: GuideCopy = {
  lang: 'es',
  path: '/es/comprimir-video-a-un-tamano/',
  title: 'Comprimir un vídeo a un tamaño concreto en Mac, sin subirlo | Ferrocast',
  description:
    'Reduce un vídeo para que quepa en los 20 MiB de Discord o los 25 MB de Gmail, en tu Mac y sin subirlo. Ferrocast calcula el bitrate desde el tamaño. Gratis.',
  crumbs: [['Inicio', '/es/'], ['Comprimir un vídeo a un tamaño', '/es/comprimir-video-a-un-tamano/']],
  h1: 'Comprimir un vídeo a un tamaño concreto en tu Mac',
  lead: [
    'Escribe el tamaño que necesitas y Ferrocast calcula el bitrate a partir de la duración del vídeo; después guarda un MP4 más pequeño junto al original. No se sube nada. Ferrocast es gratis y de código abierto (MIT) y usa ffmpeg en tu Mac para el vídeo.',
    'Apunta a ese tamaño, pero no lo garantiza al byte. En nuestra prueba el resultado quedó hasta un 0,7 % por encima del objetivo, así que deja un pequeño margen bajo el límite.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · necesita ffmpeg para vídeo' },
  limits: {
    h: 'Qué tamaño escribir',
    caption: 'Límites de subida y tamaño objetivo que escribir en Ferrocast',
    cols: ['Adónde va', 'Límite', 'Escribe en Ferrocast'],
    rows: [
      ['Discord, sin Nitro', '20 MiB por archivo (desde el 3 de septiembre de 2026; antes, 10 MiB)', '`19000 KB`'],
      ['Adjunto de Gmail', '25 MB en total; por encima, Gmail añade un enlace de Google Drive', '`23000 KB`'],
    ],
    note: 'Fuentes: [referencia para desarrolladores de Discord](https://github.com/discord/discord-api-docs/blob/main/developers/reference.mdx) y [Ayuda de Gmail](https://support.google.com/mail/answer/6584), comprobadas el 27 de octubre de 2026. El campo de tamaño de Ferrocast va en KB (1 KB = 1024 bytes), así que 19000 KB son unos 18,6 MiB.',
  },
  steps: {
    h: 'Cómo hacerlo',
    howToName: 'Comprimir un vídeo a un tamaño concreto en Mac con Ferrocast',
    items: [
      ['Instala Ferrocast y ffmpeg', 'Instala Ferrocast (DMG o Homebrew) y ffmpeg con brew install ffmpeg. La compresión de vídeo necesita ffmpeg.'],
      ['Selecciona el vídeo', 'Selecciona el vídeo en el lanzador, o arrástralo desde el Finder y pulsa ⌘ para abrir la rueda de herramientas. Puedes seleccionar varios vídeos.'],
      ['Abre Compress…', 'Elige Compress… y pon el objetivo en Size.'],
      ['Escribe el tamaño objetivo', 'Escribe el tamaño en KB, por ejemplo 19000 para Discord. La ventana muestra los bitrates que usará y rechaza un objetivo demasiado pequeño para la duración del vídeo.'],
      ['Reduce la resolución si el vídeo es largo', 'Si quieres, activa Resize y fija el lado largo (por ejemplo, 1280). Nunca amplía.'],
      ['Pulsa Intro y comprueba el tamaño', 'La copia se guarda junto al original como «nombre compressed.mp4» y nunca lo sobrescribe. Comprueba su tamaño en el Finder antes de subirla.'],
    ],
    code: 'brew install ffmpeg',
  },
  sections: [
    {
      h: 'Cómo se convierte el tamaño en bitrate',
      paras: [
        'El tamaño de un vídeo es, más o menos, su bitrate por su duración. Ferrocast divide el tamaño objetivo entre la duración, reserva una parte para el audio (128 kbit/s, o 96, 64 o 32 si el margen es justo) y da el resto a la imagen. El vídeo se recodifica una vez en H.264 con audio AAC dentro de un MP4.',
        'Ejemplo: un clip de 60 segundos a 19000 KB recibe unos 2,5 Mbit/s en total. Es de sobra para una grabación de pantalla en 1080p y justo para un clip de juego con mucho movimiento.',
      ],
    },
    {
      h: 'Cuándo bajar la resolución',
      paras: ['Cuanto más largo es el vídeo, menos bits por segundo caben en el mismo tamaño. En clips largos, bajar la resolución suele verse mejor que mantener 1080p con un bitrate muy bajo.'],
      list: ['Menos de un minuto para Discord: suele bastar con el tamaño original.', 'Unos pocos minutos: prueba Resize a 1280.', 'Mucho más largo: 720 o 480, o divide antes el vídeo con Split video.'],
    },
    {
      h: 'Límites',
      paras: [],
      list: [
        'Apunta al tamaño en una sola pasada, así que el resultado puede quedar un poco por encima (hasta un 0,7 % en nuestra prueba). Deja un 5 % de margen bajo el límite.',
        'La compresión de vídeo necesita ffmpeg; Ferrocast te avisa si falta.',
        'La salida es siempre un MP4 en H.264.',
        'Escribir «25mb» en la vista Convertir solo funciona con imágenes; para vídeo usa la ventana Compress…',
        'Solo Mac con Apple Silicon.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Qué tamaño pongo para Discord?', 'Sin Nitro, Discord acepta archivos de hasta 20 MiB desde el 3 de septiembre de 2026. En Ferrocast escribe 19000 KB para dejar margen.'],
      ['¿Se sube mi vídeo a algún sitio?', 'No. Ferrocast comprime el vídeo en tu Mac con ffmpeg; no se sube nada.'],
      ['¿Queda exactamente del tamaño que pido?', 'No al byte. Ferrocast calcula el bitrate a partir del tamaño y la duración y codifica una vez, así que el resultado queda cerca del objetivo y puede pasarse un poco. Deja un 5 % de margen.'],
      ['¿Necesito ffmpeg?', 'Sí, para vídeo. Instálalo con brew install ffmpeg. Las imágenes se pueden comprimir sin él.'],
      ['¿Sobrescribe el original?', 'No. La copia más pequeña se guarda junto al original como «nombre compressed.mp4».'],
      ['¿Puedo comprimir varios vídeos a la vez?', 'Sí. Selecciona varios vídeos y abre Compress…; cada uno se comprime al mismo tamaño objetivo.'],
      ['¿Ferrocast es gratis?', 'Sí. Ferrocast es gratis y de código abierto con licencia MIT, sin cuenta ni plan de pago.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Herramienta Compress', '/tools/compress/'],
      ['Dividir un vídeo', '/tools/split-video/'],
      ['Recortar un vídeo', '/tools/trim-video/'],
      ['Conversiones de vídeo', '/convert/video/'],
      ['Herramientas opcionales (ffmpeg)', '/docs/files/optional-tools/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
    ],
  },
};
