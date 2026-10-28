/*
 * Copy for /es/captura-de-pantalla-con-scroll-mac/ (Spanish only, board idea "Hacer una captura de pantalla
 * con scroll (página completa) en Mac gratis" and copying the text of a capture). Ferrocast facts come from
 * docs/launcher/screen-capture.md (and its Spanish version), crates/macos/src/scroll.rs (vertical scroll
 * events: a negative dy reveals what is below) and crates/app/src/capture_ctl.rs. The Apple screenshot
 * guide (Mac User Guide, "Take a screenshot") was read on 2026-10-28: it lists Shift-Command-3, 4, 5 and 6
 * and does not mention scrolling capture, so the page only says that. Rules: no claim that macOS "cannot";
 * no comparison with other apps; Spanish from Spain, tú.
 */
import type { GuideCopy } from './compress-video';

export const CHECKED = '2026-10-28';

const APPLE = 'https://support.apple.com/es-es/guide/mac-help/mh26782/mac';

export const page: GuideCopy = {
  lang: 'es',
  path: '/es/captura-de-pantalla-con-scroll-mac/',
  title: 'Captura de pantalla con scroll en Mac, gratis | Ferrocast',
  description:
    'Haz una captura larga (con scroll) de una web, un chat o un documento en tu Mac y copia el texto de la captura sin subirla. Gratis y de código abierto.',
  crumbs: [['Inicio', '/es/'], ['Captura con scroll', '/es/captura-de-pantalla-con-scroll-mac/']],
  h1: 'Cómo hacer una captura de pantalla con scroll en Mac, gratis',
  lead: [
    'Con Ferrocast eliges el área que se desplaza, la app la desplaza por ti, toma una tira tras cada paso y las une en una sola imagen alta. Se detiene cuando el contenido deja de moverse o tras 60 pantallas. Después puedes copiar el texto de la imagen sin que salga de tu Mac.',
    'La guía de Apple sobre capturas de pantalla ([Apple](' + APPLE + ')) describe ⇧⌘3, ⇧⌘4, ⇧⌘5 y ⇧⌘6, y no menciona la captura con desplazamiento. Ferrocast es gratis y de código abierto (MIT); escribo Ferrocast y no está afiliado a Apple.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · macOS 14 o posterior' },
  limits: {
    h: 'Qué necesitas y qué obtienes',
    caption: 'Requisitos y resultado de la captura con scroll de Ferrocast',
    cols: ['', 'Captura con scroll de Ferrocast'],
    rows: [
      ['Comando', 'Capture Scrolling, desde el lanzador (⌥ Espacio)'],
      ['Sistema', 'macOS 14 o posterior y un Mac con Apple Silicon'],
      ['Permisos', 'Grabación de pantalla, y Accesibilidad porque Ferrocast envía los eventos de scroll'],
      ['Atajo', 'Ninguno por defecto: lo eliges en Ajustes → Capture → Shortcuts'],
      ['Cuándo se detiene', 'Cuando dos tiras seguidas son idénticas o tras 60 pantallas'],
      ['Resultado', 'Un PNG a tamaño nativo en `~/Pictures/Ferrocast Captures`; el puntero no se dibuja en la imagen'],
      ['Después', 'Copiar, guardar, anotar, poner un fondo, copiar el texto o fijarla sobre otras ventanas'],
    ],
    note: 'Fuente: la [documentación de captura de pantalla](/es/docs/launcher/screen-capture/) y el código de la app. Guía de Apple comprobada el 28 de octubre de 2026.',
  },
  steps: {
    h: 'Cómo hacer la captura',
    howToName: 'Hacer una captura de pantalla con scroll en Mac con Ferrocast',
    items: [
      ['Instala Ferrocast y concede los permisos', 'Descarga el DMG o instálalo con Homebrew. La primera vez macOS te pedirá Grabación de pantalla, y la captura con scroll necesita además Accesibilidad.'],
      ['Prepara la ventana', 'Deja visible la web, el chat o el documento que quieres capturar, con el contenido al principio.'],
      ['Ejecuta Capture Scrolling', 'Abre el lanzador con ⌥ Espacio, ejecuta Capture Scrolling y elige el área que se desplaza.'],
      ['Espera a que termine', 'Ferrocast desplaza el área hacia abajo y une las tiras. Esc la cancela; hacer clic no.'],
      ['Usa la captura', 'En la ventana de captura: Copy (↵), Save (⌘S), Annotate… (⌘⇧A), Copy Text (⌘T) o Pin (⌘P).'],
    ],
  },
  sections: [
    {
      h: 'Copiar el texto de una captura',
      paras: [
        'Pulsa ⌘T en la ventana de captura. Copy Text lee el texto con el framework Vision de macOS, en tu Mac, y lo deja en el portapapeles en orden de lectura. Un aviso dice «Copied N characters» o «No text found». La imagen no se envía a ningún sitio.',
        'No he documentado qué idiomas lee ni si lee códigos QR, así que no lo afirmo.',
      ],
    },
    {
      h: 'Si la captura sale cortada',
      list: [
        'Si Ferrocast pierde la referencia del solape entre dos tiras, guarda las pantallas capturadas hasta ese punto y te lo avisa.',
        'Al llegar a 60 pantallas se detiene y muestra «Scrolling capture stopped at 60 screens»; para un contenido más largo, haz otra captura desde donde termina.',
        'Ferrocast envía eventos de scroll verticales, así que esta función sirve para contenido que se desplaza hacia abajo; no afirmo que funcione en horizontal.',
      ],
    },
    {
      h: 'Límites',
      list: [
        'Solo PNG. La captura con scroll guarda todas las tiras en memoria hasta terminar.',
        'Necesita macOS 14 o posterior; en macOS 13 los comandos de captura están ocultos.',
        'Solo Mac con Apple Silicon.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Cómo hago una captura de pantalla completa de una web larga en Mac?', 'Con Ferrocast: ejecuta Capture Scrolling, elige el área que se desplaza y la app la desplaza y une las tiras en un solo PNG. Se detiene cuando el contenido deja de moverse o tras 60 pantallas.'],
      ['¿Es gratis?', 'Sí. Ferrocast es gratis y de código abierto con licencia MIT y no tiene plan de pago.'],
      ['¿Qué permisos necesita?', 'Grabación de pantalla, y Accesibilidad para la captura con scroll, porque Ferrocast envía los eventos de scroll.'],
      ['¿Puedo copiar el texto de una captura?', 'Sí. En la ventana de captura pulsa ⌘T: Ferrocast lee el texto con Vision en tu Mac y lo copia al portapapeles. La imagen no se sube a ningún sitio.'],
      ['¿Hay un atajo de teclado?', 'No por defecto. Puedes asignar uno a cada comando de captura en Ajustes → Capture → Shortcuts.'],
      ['¿Funciona en un Mac con macOS 13 o con Intel?', 'No. La captura necesita macOS 14 o posterior y un Mac con Apple Silicon.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Grabar la pantalla con zoom automático', '/es/grabar-pantalla-mac-zoom-automatico/'],
      ['Documentación de captura de pantalla', '/es/docs/launcher/screen-capture/'],
      ['Alternativa gratis a CleanShot X (en inglés)', '/cleanshot-alternative/'],
      ['Difuminar parte de un vídeo', '/es/difuminar-video-mac/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
