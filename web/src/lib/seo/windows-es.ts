/*
 * Copy for /es/gestion-de-ventanas-mac/ (Spanish, sister of /mac-window-management/). The command list is
 * COMMANDS from windows.ts (a test checks it against crates/macos/src/windows.rs). Built-in macOS tiling
 * facts come from Apple Support (Mac window tiling icons & keyboard shortcuts), Rectangle facts from
 * rectangleapp.com and Magnet facts from magnet.crowdcafe.com, all read on 28 October 2026. What those pages
 * do not state is written as "no consta" (never "no tiene"). Rules: no rival speed or memory numbers; Magnet's
 * price is not stated on its page, so it is not stated here.
 */
import type { GuideCopy } from './compress-video';
import { COMMANDS } from './windows';
import { languageFaqEs } from './interface-language';

export const CHECKED = '2026-10-28';

const TILING = 'https://support.apple.com/es-es/guide/mac-help/mchl9674d0b0/mac';
const RECT = 'https://rectangleapp.com/';
const MAGNET = 'https://magnet.crowdcafe.com/';

/** The English names of the commands, as they appear in the launcher. */
export const COMMAND_LIST = COMMANDS.join(' · ');

export const page: GuideCopy = {
  lang: 'es',
  path: '/es/gestion-de-ventanas-mac/',
  title: 'Organizar ventanas en Mac: alternativa a Rectangle | Ferrocast',
  description: `Mueve y redimensiona ventanas del Mac con ${COMMANDS.length} comandos (mitades, tercios, cuartos, sextos, pantallas) desde un lanzador gratis y de código abierto.`,
  crumbs: [['Inicio', '/es/'], ['Gestión de ventanas', '/es/gestion-de-ventanas-mac/']],
  h1: `Organizar ventanas en Mac con atajos: ${COMMANDS.length} comandos en tu lanzador`,
  lead: [
    `Ferrocast tiene ${COMMANDS.length} comandos que mueven y redimensionan la ventana de delante: mitades, tercios, cuartos, sextos, maximizar, centrar, desplazar y pasar a otra pantalla. Escribes su nombre en el lanzador o le asignas un atajo propio con ⇧⌘, sobre el comando. Es gratis y de código abierto (MIT). Puedes usarlo con la interfaz en español.`,
    `Puede que no necesites otra app: macOS ya tiene mosaico de ventanas con el botón verde, el menú Ventana y atajos con Fn-Control ([Apple](${TILING})). Rectangle y Magnet son gestores de ventanas dedicados. Ferrocast es para quien prefiere tenerlo dentro del lanzador. Escribo Ferrocast y no está afiliado a Apple, Rectangle ni Magnet.`,
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · sin cuenta' },
  limits: {
    h: 'Cuatro formas de organizar ventanas en un Mac',
    caption: 'Mosaico de macOS, Rectangle, Magnet y Ferrocast',
    cols: ['', 'macOS (de serie)', 'Rectangle', 'Magnet', 'Ferrocast'],
    rows: [
      ['Cómo se usa', 'Botón verde, menú Ventana, atajos con Fn-Control', 'Atajos de teclado o zonas de anclaje al arrastrar', 'Arrastrar a los bordes, atajos personalizables o menú de la barra', 'Escribes el comando en el lanzador o le das un atajo'],
      ['Disposiciones', 'Mitades, cuartos, rellenar, centrar, volver al tamaño anterior y colocaciones de varias ventanas; los tercios y sextos no aparecen en su lista', 'No comparado aquí', 'Pantalla completa, mitades, cuartos y tercios; hasta seis pantallas externas', 'Mitades, tercios, cuartos, sextos, maximizar, centrar, desplazar y pantallas'],
      ['Precio', 'Incluido en macOS', 'Descarga gratis; Rectangle Pro es de pago, con 10 días de prueba', 'No consta en su web (se ofrece en la Mac App Store)', 'Gratis (MIT)'],
      ['Requisitos', 'macOS', 'macOS 14 o posterior según su web', 'No consta en su web', 'Mac con Apple Silicon'],
      ['Además', '—', 'Gestión de ventanas', 'Gestión de ventanas', 'Lanzador, portapapeles, snippets, conversión de archivos, grabación de pantalla, chat con IA'],
    ],
    note: `macOS: [guía de Apple](${TILING}). Rectangle: [rectangleapp.com](${RECT}). Magnet: [magnet.crowdcafe.com](${MAGNET}). Comprobado el 28 de octubre de 2026.`,
  },
  steps: {
    h: 'Cómo configurarlo',
    howToName: 'Organizar ventanas en Mac desde el lanzador de Ferrocast',
    items: [
      ['Instala Ferrocast', 'Descarga el DMG o instálalo con Homebrew.'],
      ['Concede Accesibilidad', 'Los comandos de ventana necesitan el permiso de Accesibilidad; Ferrocast lo pide en el primer arranque.'],
      ['Ejecuta un comando', 'Abre el lanzador con ⌥ Espacio y escribe, por ejemplo, «left half» o «next display».'],
      ['Añade atajos', 'Selecciona un comando y pulsa ⇧⌘, para darle su propio atajo.'],
    ],
  },
  sections: [
    {
      h: 'Todos los comandos',
      paras: [
        `Los nombres de los comandos están en inglés, tal como los escribes en el lanzador: ${COMMAND_LIST}.`,
      ],
    },
    {
      h: 'Mosaico automático (opcional)',
      paras: [
        'Desde la versión 0.4.0 Ferrocast también tiene un mosaico automático que da un sitio a cada ventana nueva, con cuatro disposiciones. Viene desactivado, porque mueve tus ventanas; se activa en Ajustes → Avanzado → Window tiling. Está explicado en la [documentación del mosaico](/es/docs/launcher/tiling/).',
      ],
    },
    {
      h: 'Límites',
      list: [
        'Necesita el permiso de Accesibilidad.',
        'Las ventanas se mueven con comandos, sin zonas de anclaje al arrastrarlas a un borde (macOS lo hace por su cuenta).',
        'Solo Mac con Apple Silicon.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Puedo organizar ventanas en un Mac sin Rectangle ni Magnet?', `Sí. macOS tiene mosaico de ventanas con el botón verde, el menú Ventana y atajos con Fn-Control, y Ferrocast añade ${COMMANDS.length} comandos de ventana a su lanzador.`],
      ['¿Qué disposiciones tiene Ferrocast?', 'Mitades, tercios, dos tercios, cuartos, cuartos de esquina y sextos, además de maximizar, casi maximizar, maximizar el ancho o el alto, centrar, tamaño razonable, mover, agrandar o reducir, restaurar, pantalla siguiente o anterior, pantalla completa y minimizar.'],
      ['¿Puedo poner mis propios atajos?', 'Sí. Selecciona un comando de ventana en el lanzador y pulsa ⇧⌘, para asignarle un atajo.'],
      ['¿Qué trae macOS de serie para dividir la pantalla?', 'Según la guía de Apple, el mosaico de ventanas está en el botón verde y en el menú Ventana, con atajos de Fn-Control. Incluye mitades, cuartos, rellenar, centrar y colocaciones de varias ventanas.'],
      ['¿Ferrocast es gratis?', 'Sí. Ferrocast es gratis y de código abierto con licencia MIT.'],
    
      languageFaqEs,
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Documentación de ventanas (en inglés)', '/docs/launcher/window-management/'],
      ['Mosaico automático', '/es/docs/launcher/tiling/'],
      ['English version', '/mac-window-management/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Raycast vs Alfred vs Spotlight', '/es/raycast-vs-alfred-vs-spotlight/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
