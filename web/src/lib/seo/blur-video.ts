/*
 * Copy for /blur-video-mac/ and /es/difuminar-video-mac/. Source: the Notion brief "Difuminar vídeo en Mac".
 * Ferrocast facts from crates/core/src/redactplan.rs (64 areas, solid black or blur, per-area times),
 * crates/core/src/media.rs and crates/app/src/media_tools.rs (H.264 MP4, audio copied when present),
 * crates/core/src/convert.rs (one area in the typed `redact` video command), crates/app/src/app.rs
 * (image blur slider 2..=80). These are in release v0.3.0.
 * Competitor cells only from pages opened on 28 October 2026. Koi's older $20 / $192 / $59 figures
 * were not on the page that day and are not used. Cutscript's price was not on the blur doc.
 */

export const CHECKED = '2026-10-28';

const KOI = 'https://koiscribe.com/guides/blur-part-of-video-mac';
const DEFACE = 'https://deface.mop.group/';
const FCP = 'https://www.apple.com/final-cut-pro/';
const FCP_GUIDE = 'https://support.apple.com/guide/final-cut-pro/welcome/mac';
const QT = 'https://support.apple.com/guide/quicktime-player/welcome/mac';
const IMOVIE = 'https://apps.apple.com/us/app/imovie/id377298193';
const CUTSCRIPT = 'https://cutscript.dev/docs/edit/blur';

export interface BlurCopy {
  lang: 'en' | 'es';
  path: string;
  title: string;
  description: string;
  crumbs: [string, string][];
  h1: string;
  lead: string[];
  cta: { dmg: string; note: string };
  before: { h: string; yesLabel: string; noLabel: string; yes: string[]; no: string[] };
  steps: { h: string; howToName: string; items: [string, string][] };
  command: { h: string; paras: string[] };
  moving: { h: string; paras: string[] };
  photos: { h: string; paras: string[] };
  table: { h: string; caption: string; cols: string[]; rows: string[][]; note: string };
  faq: { h: string; items: [string, string][] };
  more: { h: string; links: [string, string][] };
  closing: string;
}

export const en: BlurCopy = {
  lang: 'en',
  path: '/blur-video-mac/',
  title: 'Blur Part of a Video on Mac, Free and Offline | Ferrocast',
  description:
    'Cover up to 64 areas of a video on your Mac, each with its own start and end time, with blur or a solid box. Free, local, needs ffmpeg. No face tracking.',
  crumbs: [['Home', '/'], ['Files', '/files/'], ['Blur part of a video', '/blur-video-mac/']],
  h1: 'Blur part of a video on your Mac, without uploading it',
  lead: [
    'Ferrocast covers up to 64 areas of a video, each with its own start and end, with blur or a solid black box. It runs on your Mac and uploads nothing. It needs ffmpeg (`brew install ffmpeg`). It does not follow a face or an object that moves: each area stays where you place it.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · macOS 13 or later · needs ffmpeg' },
  before: {
    h: 'Before you start: what it does and what it does not',
    yesLabel: 'It does',
    noLabel: 'It does not',
    yes: [
      'Several areas in one pass, up to 64, each with its own start and end in seconds.',
      'Blur, or a solid black box. The black is not a colour you pick, and the blur strength is fixed in the encoder. The 2–80 px blur slider is for photos, not video.',
      'mp4, mov and mkv in. The copy is always an H.264 MP4 next to the original. The picture is re-encoded, so this is not a promise of the same quality. If the file has an audio track, that audio is copied rather than re-encoded.',
    ],
    no: [
      'Tracking a face or an object through the video. The area does not move with the subject.',
      'A preview of the finished blur. The window only marks the areas on a reference frame. Do not expect the export to be drawn in the window before you press ↵.',
    ],
  },
  steps: {
    h: 'How to blur part of a video on a Mac',
    howToName: 'Blur part of a video on a Mac with Ferrocast',
    items: [
      ['Install ffmpeg once', 'Run brew install ffmpeg. If ffmpeg is missing, Ferrocast shows that command instead of failing quietly. Video redaction does not run without it. The same line is in [Optional tools](/docs/files/optional-tools/).'],
      ['Open Redact Video', 'There is no keyboard shortcut. Drag the video and tap ⌘ to open the Tools wheel, then choose Redact Video. You can also open it from the editors in the Convert view.'],
      ['Add areas and set each one’s time', 'Each area is a rectangle you drag. Set its start and end in seconds, and choose Solid or Blur. The Reference frame slider picks which frame you see while you place the boxes. Overlapping areas are allowed, and a rotated video is handled in the displayed size.'],
      ['Save the copy', 'Press ↵. Ferrocast writes a new file next to the original, with “redacted” before .mp4, and never overwrites the original. The container is MP4 even when the source was MOV or MKV.'],
    ],
  },
  command: {
    h: 'Do it with one command',
    paras: [
      '`redact 10,10 200x100 0:05-0:09 blur` covers a 200×100 pixel area whose top-left corner is at x 10, y 10, from 5 seconds to 9 seconds, with blur. Without the word `blur`, the same rectangle is a solid black box.',
      'The video command takes one area. The parser does not accept several areas on one line. For more than one area, use the Redact Video window. (On a photo, the typed command can list several rectangles; that is a different tool.)',
    ],
  },
  moving: {
    h: 'What if the face moves?',
    paras: [
      'Ferrocast does not track faces or objects in a video. If the person walks across the frame, the box stays put and the face leaves it. I would rather say that here than after you have installed it.',
      'You can split the time into short ranges and place a new area for each range. The limit is 64 areas, and each one has its own start and end. That is a workaround for a short shift, not a tracker, and it is not practical for a long shot of someone moving.',
      `For a moving face, use an editor that tracks. Apple’s [Final Cut Pro user guide](${FCP_GUIDE}) includes object tracking (“Track an effect to a moving object”). [Cutscript](${CUTSCRIPT}) can follow a blur and can find faces and emails with Vision on a Mac; its blur page does not show a price. [Deface](${DEFACE}) blurs detected faces offline on a Mac (Apple Silicon or Intel), is free, and says detection can miss people.`,
    ],
  },
  photos: {
    h: 'For photos, it does detect faces and text',
    paras: [
      'This section is for still images, not video. [Redact photo](/tools/redact-image/) (⇧⌘R) finds faces or text on your Mac with Vision, or you draw the areas yourself. The cover is a solid colour or a blur from 2 to 80 pixels. Inputs are jpg, png, webp, heic and tiff. Typed commands: `redact faces`, `redact text blur 25`.',
      'Nothing in that window follows a face through a video. If the file is a video, use Redact Video and place the boxes yourself.',
    ],
  },
  table: {
    h: 'A short comparison',
    caption: 'Blur tools compared with facts checked on 28 October 2026',
    cols: ['', 'Ferrocast Redact Video', 'iMovie / QuickTime', 'Koi', 'Deface', 'Final Cut Pro'],
    rows: [
      ['Price', 'Free (MIT)', `iMovie is free on the [Mac App Store](${IMOVIE}). QuickTime Player ships with macOS.`, `No free plan. Pro is $15 a month, or $1 for the first 3 days. Lifetime is a one-time purchase; the guide does not show that amount. [Koi’s guide](${KOI}).`, 'Free', `$299.99 once, or Apple Creator Studio at $12.99 a month or $129 a year. [apple.com/final-cut-pro](${FCP}).`],
      ['Uploads the video', 'No', 'Not stated on the Apple pages opened. Both are Mac apps.', 'Koi’s guide says no', 'The homepage says no uploads', 'A Mac app; the pricing page does not discuss uploads'],
      ['Several areas, each with its own time', 'Yes, up to 64', `Not described in the [QuickTime Player guide](${QT}) that was opened (it describes trim, rearrange, rotate and split).`, 'The guide says the boxes stay for the whole video', 'Automatic faces; per-area times are not on the homepage', 'Not quoted here. The user guide lists masking and object tracking.'],
      ['Follows a moving face or object', 'No', 'Not described in that QuickTime guide', 'The guide says no: the boxes stay put', 'Automatic face detection. They warn it can miss people. Motion tracking is not described on the homepage.', 'Yes. The user guide includes “Track an effect to a moving object”.'],
      ['Requirements', 'Apple Silicon, macOS 13 or later, ffmpeg', 'macOS', 'Not stated on the blur guide', 'Mac, Apple Silicon or Intel', 'macOS 15.6 or later, 8 GB of memory, 7.2 GB of disk. Some features need Apple silicon.'],
    ],
    note: `Checked 28 October 2026. Koi from its own guide, Deface from its homepage, Final Cut Pro from apple.com and the user guide, iMovie from the US App Store, QuickTime from Apple’s user guide. Screenify’s price was not opened and is not in this table. Ferrocast is independent and is not affiliated with Apple or the other products named here.`,
  },
  faq: {
    h: 'Questions',
    items: [
      ['Can I blur part of a video on a Mac for free?', 'Yes. Ferrocast, a free open-source Mac app, covers up to 64 areas of an mp4, mov or mkv video with blur or a solid black box, each area with its own start and end time. It runs locally and needs ffmpeg (`brew install ffmpeg`). It does not track moving faces.'],
      ['Does it follow a face that moves?', 'No. In a video, each area stays where you put it for its time range. Automatic face and text detection works on photos only. To follow a moving face you need an editor with tracking, such as Final Cut Pro.'],
      ['Is the video uploaded anywhere?', 'No. Ferrocast processes the video on your Mac with ffmpeg and saves a new copy next to the original, without overwriting it.'],
      ['Which formats does it accept, and does it overwrite the original?', 'It accepts mp4, mov and mkv. The copy is an H.264 MP4 named with “redacted”, next to the original. It never overwrites the original. The picture is re-encoded; if the file has audio, that audio is copied.'],
      ['Do I need ffmpeg?', 'Yes. Install it with brew install ffmpeg. If it is missing, Ferrocast shows that command.'],
      ['What about a photo?', 'Redact photo (⇧⌘R) can detect faces and text on the Mac, or you draw areas, with a solid colour or a blur from 2 to 80 pixels. That is for jpg, png, webp, heic and tiff, not for video.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Redact Video', '/tools/redact-video/'],
      ['Redact Video reference', '/docs/files/tools/redact-video/'],
      ['Redact photo', '/tools/redact-image/'],
      ['Optional tools (ffmpeg)', '/docs/files/optional-tools/'],
      ['Compress a video to a target size', '/compress-video-to-size/'],
      ['All file tools', '/files/'],
      ['Key facts', '/facts/'],
    ],
  },
  closing: 'Download Ferrocast, install ffmpeg once, and open Redact Video from the Tools wheel. The limits are in the [Redact Video reference](/docs/files/tools/redact-video/).',
};

export const es: BlurCopy = {
  lang: 'es',
  path: '/es/difuminar-video-mac/',
  title: 'Difuminar parte de un vídeo en Mac, gratis y sin subirlo',
  description:
    'Tapa hasta 64 zonas de un vídeo en tu Mac, cada una con su inicio y su fin, con desenfoque o un bloque de color. Gratis, local, necesita ffmpeg. Sin seguimiento de caras.',
  crumbs: [['Inicio', '/es/'], ['Difuminar parte de un vídeo', '/es/difuminar-video-mac/']],
  h1: 'Difuminar parte de un vídeo en Mac sin subirlo a internet',
  lead: [
    'Ferrocast tapa hasta 64 zonas de un vídeo, cada una con su inicio y su fin, con desenfoque o un bloque negro sólido. Lo hace en tu Mac y no sube el archivo. Necesita ffmpeg (`brew install ffmpeg`). No sigue una cara ni un objeto que se mueve: la zona se queda donde la pones.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · macOS 13 o posterior · necesita ffmpeg' },
  before: {
    h: 'Antes de empezar: lo que hace y lo que no',
    yesLabel: 'Sí',
    noLabel: 'No',
    yes: [
      'Varias zonas a la vez, hasta 64, cada una con sus segundos de inicio y de fin.',
      'Desenfoque, o un bloque negro. El negro no se elige, y la fuerza del desenfoque en vídeo es fija. El deslizador de 2 a 80 px es para fotos, no para vídeo.',
      'Entra mp4, mov o mkv. La copia es siempre un MP4 en H.264 junto al original. La imagen se recodifica, así que no prometo la misma calidad. Si el archivo tiene pista de audio, ese audio se copia, no se vuelve a codificar.',
    ],
    no: [
      'Seguir una cara o un objeto a lo largo del vídeo. La zona no se mueve con la persona.',
      'Una vista previa del resultado. La ventana solo marca las zonas sobre un fotograma de referencia. El desenfoque de verdad aparece en el archivo exportado, no en esa vista.',
    ],
  },
  steps: {
    h: 'Cómo difuminar parte de un vídeo en Mac',
    howToName: 'Difuminar parte de un vídeo en Mac con Ferrocast',
    items: [
      ['Instala ffmpeg una vez', 'Ejecuta brew install ffmpeg. Si falta, Ferrocast muestra ese comando en vez de fallar en silencio. Sin ffmpeg, Redact Video no se ejecuta. La misma línea está en [Herramientas opcionales](/docs/files/optional-tools/) (en inglés).'],
      ['Abre Redact Video', 'No tiene atajo. Arrastra el vídeo y pulsa ⌘ para abrir la rueda de herramientas, y elige Redact Video. También puedes abrirlo desde los editores de la vista Convert.'],
      ['Añade zonas y ajusta el tiempo de cada una', 'Cada zona es un rectángulo que arrastras. Pones el inicio y el fin en segundos, y eliges Solid o Blur. El deslizador Reference frame elige qué fotograma ves para colocar las cajas. Las zonas pueden solaparse, y un vídeo girado se trata en el tamaño en que se ve.'],
      ['Guarda la copia', 'Pulsa ↵. Ferrocast escribe un archivo nuevo junto al original, con «redacted» antes de .mp4, y nunca sobrescribe el original. El contenedor es MP4 aunque el original fuera MOV o MKV.'],
    ],
  },
  command: {
    h: 'Hacerlo con un comando',
    paras: [
      '`redact 10,10 200x100 0:05-0:09 blur` tapa una zona de 200×100 píxeles cuya esquina superior izquierda está en x 10, y 10, del segundo 5 al 9, con desenfoque. Sin la palabra `blur`, el mismo rectángulo es un bloque negro.',
      'El comando de vídeo admite una sola zona. El analizador no acepta varias zonas en la misma línea. Para más de una, usa la ventana Redact Video. En una foto, el comando escrito sí puede enumerar varios rectángulos; es otra herramienta.',
    ],
  },
  moving: {
    h: '¿Y si la cara se mueve?',
    paras: [
      'Ferrocast no hace seguimiento en vídeo. Si la persona cruza el encuadre, la caja se queda quieta y la cara se sale. Prefiero decirlo aquí, antes de que instales nada.',
      'Puedes partir el tiempo en tramos cortos y poner una zona en cada tramo. El límite es 64 zonas, y cada una tiene su inicio y su fin. Sirve para un desplazamiento breve, no es un tracker, y no es práctico en un plano largo.',
      `Si la cara se mueve, hace falta un editor con tracking. La [guía de Final Cut Pro](${FCP_GUIDE}) incluye el seguimiento de objetos («Track an effect to a moving object»). [Cutscript](${CUTSCRIPT}) puede seguir un desenfoque y localizar caras y correos con Vision en el Mac; en esa página no aparece el precio. [Deface](${DEFACE}) difumina caras detectadas, sin conexión, en Mac con Apple Silicon o Intel, es gratis y avisa de que la detección puede fallar.`,
    ],
  },
  photos: {
    h: 'En las fotos sí detecta caras y texto',
    paras: [
      'Esto es para fotos, no para vídeo. [Redactar imagen](/tools/redact-image/) (⇧⌘R) detecta caras o texto en tu Mac con Vision, o dibujas las zonas a mano. La tapa es un color sólido o un desenfoque de 2 a 80 píxeles. Acepta jpg, png, webp, heic y tiff. Comandos: `redact faces`, `redact text blur 25`.',
      'Esa ventana no sigue una cara dentro de un vídeo. Si el archivo es un vídeo, usa Redact Video y coloca las cajas tú.',
    ],
  },
  table: {
    h: 'Comparativa rápida',
    caption: 'Herramientas para tapar zonas de un vídeo, con datos comprobados el 28 de octubre de 2026',
    cols: ['', 'Ferrocast Redact Video', 'iMovie / QuickTime', 'Koi', 'Deface', 'Final Cut Pro'],
    rows: [
      ['Precio', 'Gratis (MIT)', `iMovie es gratis en la [App Store de EE. UU.](${IMOVIE}). QuickTime Player viene con macOS.`, `Sin plan gratis. Pro cuesta 15 $ al mes, o 1 $ los 3 primeros días. La licencia de por vida es un pago único; la guía no muestra esa cifra. [Guía de Koi](${KOI}).`, 'Gratis', `299,99 $ de una vez, o Apple Creator Studio a 12,99 $ al mes o 129 $ al año. [apple.com/final-cut-pro](${FCP}).`],
      ['Sube el vídeo', 'No', 'Las páginas de Apple que abrí no lo dicen. Las dos son apps de Mac.', 'La guía de Koi dice que no', 'Su página dice que no hay subidas', 'Es una app de Mac; la página de precio no habla de subidas'],
      ['Varias zonas con tiempo propio', 'Sí, hasta 64', `No aparece en la [guía de QuickTime Player](${QT}) que abrí (describe recortar, reordenar, girar y dividir).`, 'La guía dice que las cajas se quedan durante todo el vídeo', 'Caras automáticas; los tiempos por zona no están en la página de inicio', 'No lo cito. La guía incluye máscaras y seguimiento de objetos.'],
      ['Sigue una cara u objeto que se mueve', 'No', 'No aparece en esa guía de QuickTime', 'La guía dice que no: las cajas se quedan quietas', 'Detección automática de caras. Avisan de que puede fallar. El seguimiento del movimiento no está descrito en la página de inicio.', 'Sí. La guía incluye «Track an effect to a moving object».'],
      ['Requisitos', 'Apple Silicon, macOS 13 o posterior, ffmpeg', 'macOS', 'No consta en la guía de desenfoque', 'Mac con Apple Silicon o Intel', 'macOS 15.6 o posterior, 8 GB de memoria, 7,2 GB de disco. Algunas funciones piden Apple silicon.'],
    ],
    note: `Comprobado el 28 de octubre de 2026. Koi en su guía, Deface en su página, Final Cut Pro en apple.com y en la guía de usuario, iMovie en la App Store de EE. UU. y QuickTime en la guía de Apple. El precio de Screenify no se abrió y no está en la tabla. Ferrocast es independiente y no está afiliado a Apple ni a los productos citados.`,
  },
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Puedo difuminar parte de un vídeo en Mac gratis?', 'Sí. Ferrocast, una app gratis y de código abierto para Mac, tapa hasta 64 zonas de un vídeo mp4, mov o mkv con desenfoque o un bloque negro, cada zona con su inicio y su fin. Se ejecuta en tu Mac y necesita ffmpeg (`brew install ffmpeg`). No sigue caras en movimiento.'],
      ['¿Ferrocast sigue una cara que se mueve?', 'No. En vídeo, cada zona se queda en el sitio donde la colocas durante su intervalo de tiempo. La detección automática de caras y texto solo funciona en fotos. Para seguir una cara en movimiento hace falta un editor con tracking, como Final Cut Pro.'],
      ['¿Se sube el vídeo a algún servidor?', 'No. Ferrocast procesa el vídeo en tu Mac con ffmpeg y guarda una copia nueva junto al original, sin sobrescribirlo.'],
      ['¿Qué formatos acepta y sobrescribe el original?', 'Acepta mp4, mov y mkv. La copia es un MP4 en H.264, con «redacted» en el nombre, junto al original. Nunca lo sobrescribe. La imagen se recodifica; si hay audio, ese audio se copia.'],
      ['¿Necesito ffmpeg?', 'Sí. Instálalo con brew install ffmpeg. Si falta, Ferrocast muestra ese comando.'],
      ['¿Y en una foto?', 'Redactar imagen (⇧⌘R) puede detectar caras y texto en el Mac, o dibujas las zonas, con un color sólido o un desenfoque de 2 a 80 píxeles. Es para jpg, png, webp, heic y tiff, no para vídeo.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Redact Video', '/tools/redact-video/'],
      ['Referencia de Redact Video (en inglés)', '/docs/files/tools/redact-video/'],
      ['Redactar una foto', '/tools/redact-image/'],
      ['Herramientas opcionales, ffmpeg (en inglés)', '/docs/files/optional-tools/'],
      ['Comprimir un vídeo a un tamaño', '/es/comprimir-video-a-un-tamano/'],
      ['Todas las herramientas de archivos', '/es/files/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
  closing: 'Descarga Ferrocast, instala ffmpeg una vez y abre Redact Video desde la rueda de herramientas. Los límites están en la [referencia de Redact Video](/docs/files/tools/redact-video/) (en inglés).',
};
