/*
 * Copy for /screen-studio-alternative/ (EN) and /es/grabar-pantalla-mac-zoom-automatico/ (ES), board
 * ideas "Free Screen Studio alternative with auto zoom" and "Grabar la pantalla del Mac con zoom
 * automático gratis". Ferrocast facts come from web/src/content/docs/docs/launcher/screen-capture.md.
 * Screen Studio facts come only from screen.studio, read on CHECKED. Rules: no quality or speed
 * comparison with Screen Studio; its prices only as its website states them.
 */
import type { GuideCopy } from './compress-video';
import { languageFaqEs, languageFaqEn } from './interface-language';

export const CHECKED = '2026-10-27';

export const en: GuideCopy = {
  lang: 'en',
  path: '/screen-studio-alternative/',
  title: 'Free Screen Studio alternative with auto zoom for Mac | Ferrocast',
  description:
    'Record your Mac screen with automatic zoom on clicks, a webcam overlay and an on-device transcript, then export MP4 or GIF. Free and open source, in a launcher.',
  crumbs: [['Home', '/'], ['Screen Studio alternative', '/screen-studio-alternative/']],
  h1: 'A free screen recorder with auto zoom, built into a launcher',
  lead: [
    'Ferrocast records your screen, an area or a window, then opens the recording in an editor that zooms toward your clicks automatically. You can add your camera, clean up the cursor, transcribe the speech on your Mac and export an MP4 or a GIF. It is free and open source (MIT), and it lives in the same launcher you use for apps, clipboard and files.',
    'Screen Studio is a paid app: its website lists $9 a month billed yearly or $29 monthly (checked 27 October 2026), with features such as shareable links and a cloud transcript that Ferrocast does not have. If you need those, it is the better pick.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · macOS 14 or later' },
  limits: {
    h: 'What the recorder and editor do',
    caption: 'Ferrocast screen recorder features',
    cols: ['Part', 'In Ferrocast'],
    rows: [
      ['Record', 'A whole display, an area or a window; 3 s countdown; 60 fps (or 30).'],
      ['Auto zoom', 'Zooms toward your clicks: starts 0.4 s before the first click of a group and returns after 1.2 s without clicks or cursor movement. Manual zooms and a loupe too.'],
      ['Cursor', 'Redrawn from the recorded events: size 0.5× to 4×, smoothing, click ring, motion blur, or hidden.'],
      ['Camera and audio', 'Camera as an overlay or split layout (circle, rounded or square), microphone and system audio, gain per track.'],
      ['Transcript', 'Made on your Mac. Cut words from the text, remove fillers (English and Spanish) and tighten pauses.'],
      ['Style', 'Aspect 16:9, 4:3, 1:1 or 9:16; colour, gradient or blurred background; padding, corners and shadow.'],
      ['Export', 'MP4 (H.264 or HEVC, up to 4K, 30 or 60 fps) or GIF (480, 720 or 960 px wide, 10 or 15 fps).'],
      ['Requirements', 'macOS 14 or later, the Screen Recording permission, an Apple Silicon Mac.'],
    ],
    note: 'From the [screen capture docs](/docs/launcher/screen-capture/). Screen Studio prices from [screen.studio](https://screen.studio/), checked 27 October 2026.',
  },
  steps: {
    h: 'Record your first video',
    howToName: 'Record the Mac screen with auto zoom in Ferrocast',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew.'],
      ['Run Record Screen', 'Open the launcher and run Record Screen, Record Area or Record Window. macOS asks for the Screen Recording permission the first time.'],
      ['Stop', 'Stop from the small recording panel or the menu bar.'],
      ['Edit', 'Open Last Recording: auto zoom is already applied. Adjust zooms, cursor, camera and style, or transcribe.'],
      ['Export', 'Press ⌘E and choose MP4 or GIF.'],
    ],
  },
  sections: [
    {
      h: 'What Ferrocast does not do',
      list: [
        'No shareable links or cloud upload: exports stay on your Mac.',
        'GIF exports are limited in length (about 20 seconds at 960 px and 15 fps, about 80 seconds at 480 px).',
        'No Intel Macs and nothing before macOS 14 for recording.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is there a free Screen Studio alternative for Mac?', 'Yes. Ferrocast is a free, open-source Mac launcher with a screen recorder that zooms toward your clicks automatically and exports MP4 or GIF.'],
      ['Does Ferrocast upload my recordings?', 'No. Recordings, edits, transcripts and exports stay on your Mac. There are no shareable links.'],
      ['Can I add my webcam?', 'Yes. Turn on Camera in Settings → Capture → Sources; in the editor it can be an overlay or a split layout.'],
      ['Which macOS do I need?', 'macOS 14 or later on an Apple Silicon Mac, with the Screen Recording permission.'],
      ['Can I export a GIF?', 'Yes, at 480, 720 or 960 px wide and 10 or 15 fps, for clips up to about 20 seconds at 960 px.'],
    
      languageFaqEn,
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Screen capture docs', '/docs/launcher/screen-capture/'],
      ['Need screenshots too? A free CleanShot X alternative', '/cleanshot-alternative/'],
      ['MOV to GIF', '/convert/mov-to-gif/'],
      ['Compress a video to a target size', '/compress-video-to-size/'],
      ['Something private in the recording? Cover it', '/blur-video-mac/'],
      ['Key facts', '/facts/'],
    ],
  },
};

export const es: GuideCopy = {
  lang: 'es',
  path: '/es/grabar-pantalla-mac-zoom-automatico/',
  title: 'Grabar la pantalla del Mac con zoom automático, gratis | Ferrocast',
  description:
    'Graba la pantalla del Mac con zoom automático en los clics, tu cámara y transcripción hecha en el Mac, y exporta a MP4 o GIF. Gratis y de código abierto.',
  crumbs: [['Inicio', '/es/'], ['Grabar la pantalla con zoom', '/es/grabar-pantalla-mac-zoom-automatico/']],
  h1: 'Grabar la pantalla del Mac con zoom automático, gratis',
  lead: [
    'Ferrocast graba la pantalla, un área o una ventana y abre la grabación en un editor que hace zoom hacia tus clics automáticamente. Puedes añadir tu cámara, ajustar el cursor, transcribir lo que dices en tu propio Mac y exportar a MP4 o GIF. Es gratis y de código abierto (MIT), y vive en el mismo lanzador que usas para apps, portapapeles y archivos. Puedes usarlo con la interfaz en español.',
    'Screen Studio es una app de pago: su web indica 9 $ al mes con pago anual o 29 $ al mes (comprobado el 27 de octubre de 2026), con funciones como enlaces para compartir y transcripción en la nube que Ferrocast no tiene. Si las necesitas, es mejor opción.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · macOS 14 o posterior' },
  limits: {
    h: 'Qué hacen la grabadora y el editor',
    caption: 'Funciones de la grabadora de pantalla de Ferrocast',
    cols: ['Parte', 'En Ferrocast'],
    rows: [
      ['Grabar', 'Una pantalla entera, un área o una ventana; cuenta atrás de 3 s; 60 fps (o 30).'],
      ['Zoom automático', 'Se acerca a tus clics: empieza 0,4 s antes del primer clic de un grupo y vuelve tras 1,2 s sin clics ni movimiento del cursor. También zooms manuales y lupa.'],
      ['Cursor', 'Redibujado a partir de lo grabado: tamaño de 0,5× a 4×, suavizado, anillo de clic, desenfoque de movimiento u oculto.'],
      ['Cámara y audio', 'Cámara superpuesta o en vista dividida (círculo, redondeada o cuadrada), micrófono y audio del sistema, con ganancia por pista.'],
      ['Transcripción', 'Hecha en tu Mac. Corta palabras desde el texto, quita muletillas (inglés y español) y acorta pausas.'],
      ['Estilo', 'Formato 16:9, 4:3, 1:1 o 9:16; fondo de color, degradado o desenfocado; márgenes, esquinas y sombra.'],
      ['Exportar', 'MP4 (H.264 o HEVC, hasta 4K, 30 o 60 fps) o GIF (480, 720 o 960 px de ancho, 10 o 15 fps).'],
      ['Requisitos', 'macOS 14 o posterior, permiso de Grabación de pantalla y un Mac con Apple Silicon.'],
    ],
    note: 'Según la [documentación de captura de pantalla](/docs/launcher/screen-capture/) (en inglés). Precios de Screen Studio en [screen.studio](https://screen.studio/), comprobados el 27 de octubre de 2026.',
  },
  steps: {
    h: 'Graba tu primer vídeo',
    howToName: 'Grabar la pantalla del Mac con zoom automático en Ferrocast',
    items: [
      ['Instala Ferrocast', 'Descarga el DMG o instálalo con Homebrew.'],
      ['Ejecuta Record Screen', 'Abre el lanzador y ejecuta Record Screen, Record Area o Record Window. La primera vez, macOS pide el permiso de Grabación de pantalla.'],
      ['Detén la grabación', 'Desde el pequeño panel de grabación o desde la barra de menús.'],
      ['Edita', 'Abre Open Last Recording: el zoom automático ya está aplicado. Ajusta zooms, cursor, cámara y estilo, o transcribe.'],
      ['Exporta', 'Pulsa ⌘E y elige MP4 o GIF.'],
    ],
  },
  sections: [
    {
      h: 'Lo que Ferrocast no hace',
      list: [
        'No hay enlaces para compartir ni subida a la nube: lo exportado se queda en tu Mac.',
        'Los GIF tienen una duración limitada (unos 20 segundos a 960 px y 15 fps, unos 80 segundos a 480 px).',
        'No funciona en Mac con Intel y la grabación necesita macOS 14 o posterior.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Hay una forma gratis de grabar la pantalla del Mac con zoom automático?', 'Sí. Ferrocast es un lanzador gratis y de código abierto para Mac con una grabadora de pantalla que hace zoom hacia tus clics automáticamente y exporta a MP4 o GIF.'],
      ['¿Se suben mis grabaciones?', 'No. Las grabaciones, la edición, la transcripción y lo exportado se quedan en tu Mac. No hay enlaces para compartir.'],
      ['¿Puedo añadir mi cámara?', 'Sí. Activa Camera en Ajustes → Capture → Sources; en el editor puede ir superpuesta o en vista dividida.'],
      ['¿Qué versión de macOS necesito?', 'macOS 14 o posterior en un Mac con Apple Silicon, con el permiso de Grabación de pantalla.'],
      ['¿Puedo exportar un GIF?', 'Sí, de 480, 720 o 960 px de ancho y a 10 o 15 fps, con clips de hasta unos 20 segundos a 960 px.'],
    
      languageFaqEs,
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Convertir un vídeo a GIF', '/es/convertir/video-a-gif/'],
      ['Comprimir un vídeo a un tamaño', '/es/comprimir-video-a-un-tamano/'],
      ['¿Sale algo privado en tu grabación? Tápalo', '/es/difuminar-video-mac/'],
      ['Captura de pantalla con scroll', '/es/captura-de-pantalla-con-scroll-mac/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
