/*
 * Copy for /alfred-alternative/ (English, board idea "Free Alfred alternative (clipboard + snippets
 * without Powerpack)") and its Spanish sister /es/alternativa-a-alfred/ (`es`, board round 3). Alfred facts come only from alfredapp.com (powerpack, shop, clipboard help,
 * minimum requirements), read on CHECKED. Ferrocast facts come from its docs (clipboard, snippets,
 * script-commands, settings). Rules: no speed or memory numbers for Alfred, never "Alfred does not
 * have" without a source, and say plainly who should stay on Alfred.
 */
import type { GuideCopy } from './compress-video';

export const CHECKED = '2026-10-28';

export const page: GuideCopy = {
  lang: 'en',
  path: '/alfred-alternative/',
  title: 'Free Alfred alternative: clipboard and snippets, no Powerpack | Ferrocast',
  description:
    'Ferrocast is a free, open-source Mac launcher with clipboard history and expanding snippets built in, the two features Alfred keeps in its paid Powerpack.',
  crumbs: [['Home', '/'], ['Alfred alternative', '/alfred-alternative/']],
  h1: 'A free Alfred alternative with clipboard history and snippets',
  lead: [
    'In Alfred, clipboard history, snippets with auto-expansion and Workflows are part of the Powerpack, a one-off purchase (£34 for a single Alfred 5 licence, which the shop says includes a free upgrade to Alfred 6). Ferrocast includes clipboard history and expanding snippets for free, as an open-source (MIT) launcher for Apple Silicon Macs.',
    'Alfred has a decade of Workflows behind it and runs on older Macs; Ferrocast does not try to replace that. If what you want from the Powerpack is the clipboard and snippets, read on. I make Ferrocast, so check the sources.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Alfred Powerpack features and their Ferrocast counterpart',
    caption: 'Alfred Powerpack features compared with what Ferrocast includes for free',
    cols: ['Feature', 'Alfred', 'Ferrocast (free)'],
    rows: [
      ['Price', 'Free app; Powerpack is a one-off £34 (single Alfred 5 licence)', 'Free, no paid plan'],
      ['Clipboard history', 'Powerpack; ignores password apps by default', 'Included; skips entries from password managers; you choose how long to keep it'],
      ['Snippets with auto-expansion', 'Powerpack', 'Included, with placeholders such as `{date}`, `{clipboard}` and `{cursor}`; imports Raycast snippet JSON'],
      ['Automation', 'Workflows (Powerpack), with thousands from the community', 'Raycast-compatible [Script Commands](/raycast-script-commands/); no visual workflow editor'],
      ['Shell commands', 'Run in Terminal from Alfred (Powerpack)', 'Script Commands run without opening Terminal'],
      ['Settings sync between Macs', 'Through Dropbox or another file sync service (Powerpack)', 'Opt-in through iCloud Drive (Settings → Advanced, off by default). Keychain API keys stay on each Mac. Backup export is still in Settings'],
      ['AI', 'No built-in AI stated on its website', 'Ask AI with your provider; Ollama on your Mac by default'],
      ['File conversion', 'File actions, and custom ones through Workflows', '275 local conversions and 26 file tools'],
      ['Window management', 'Not stated on its website', 'About 40 commands'],
      ['Requirements', 'Alfred 5: macOS 10.14 or later', 'Apple Silicon, macOS 13 or later'],
      ['Source code', 'No open licence stated', 'MIT'],
    ],
    note: 'Alfred column from [alfredapp.com/powerpack](https://www.alfredapp.com/powerpack/), [the shop](https://www.alfredapp.com/shop/), [clipboard help](https://www.alfredapp.com/help/features/clipboard/) and [minimum requirements](https://www.alfredapp.com/help/getting-started/minimum-requirements/), checked 28 October 2026. Ferrocast column from its [key facts](/facts/) and docs.',
  },
  steps: {
    h: 'Move your clipboard and snippets habit to Ferrocast',
    howToName: 'Set up clipboard history and snippets in Ferrocast',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew. It needs no account.'],
      ['Pick a hotkey', 'Ferrocast opens with ⌥ Space by default; change it in Settings if Alfred uses it.'],
      ['Turn on the clipboard history', 'In Settings → Clipboard, keep Record history on and choose how long to keep it.'],
      ['Recreate your snippets', 'Open Snippets from the launcher, press ⌘N and type the keyword and text. Automatic expansion needs the Accessibility permission.'],
    ],
  },
  sections: [
    {
      h: 'Stay on Alfred if you…',
      paras: [],
      list: [
        'rely on Workflows, your own or from the community;',
        'use an Intel Mac or a macOS version older than 13;',
        'want Alfred 6, which the shop says is a free upgrade for new Alfred 5 Powerpack buyers;',
        'want settings sync through Dropbox rather than iCloud Drive.',
      ],
    },
    {
      h: 'Ferrocast fits if you…',
      paras: [],
      list: [
        'mostly want clipboard history and snippets without paying for them;',
        'want AI chat with your own model, file conversion that never uploads, or screen recording, in the same launcher;',
        'prefer software whose code you can read.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is Alfred free?', 'The Alfred app is free. Clipboard history, snippets, Workflows and settings sync are part of the Powerpack, a one-off purchase (£34 for a single Alfred 5 licence when we checked on 28 October 2026).'],
      ['Is there a free alternative to the Alfred Powerpack?', 'For clipboard history and snippets, yes. Ferrocast is a free, open-source Mac launcher that includes both. It does not run Alfred Workflows.'],
      ['Can Ferrocast import my Alfred snippets?', 'Not directly. Ferrocast imports snippets from a Raycast JSON export; Alfred snippets have to be recreated.'],
      ['Does Ferrocast run Alfred Workflows?', 'No. Ferrocast runs Raycast-compatible Script Commands, which are plain scripts with metadata comments.'],
      ['Does Ferrocast work on Intel Macs?', 'No. Ferrocast needs an Apple Silicon Mac and macOS 13 or later. Alfred 5 runs from macOS 10.14.'],
      ['Is Ferrocast affiliated with Alfred?', 'No. Ferrocast is independent and not affiliated with Alfred or Raycast.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Free clipboard manager for Mac', '/clipboard-manager-mac/'],
      ['Free text expander for Mac', '/text-expander-mac/'],
      ['Clipboard history docs', '/docs/launcher/clipboard/'],
      ['Snippets docs', '/docs/launcher/snippets/'],
      ['Run Raycast Script Commands', '/raycast-script-commands/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['Spanish version', '/es/alternativa-a-alfred/'],
      ['Key facts', '/facts/'],
    ],
  },
};

export const es: GuideCopy = {
  lang: 'es',
  path: '/es/alternativa-a-alfred/',
  title: 'Alternativa gratis a Alfred sin Powerpack para Mac | Ferrocast',
  description:
    'Ferrocast es un lanzador gratis y de código abierto para Mac con historial del portapapeles y snippets que se expanden, lo que Alfred reserva al Powerpack.',
  crumbs: [['Inicio', '/es/'], ['Alternativa a Alfred', '/es/alternativa-a-alfred/']],
  h1: 'Una alternativa gratis a Alfred, con portapapeles y snippets sin Powerpack',
  lead: [
    'En Alfred, el historial del portapapeles, los snippets con autoexpansión y los Workflows forman parte del Powerpack, una compra única (34 £ por una licencia individual de Alfred 5, que según su tienda incluye la actualización gratis a Alfred 6). Ferrocast incluye gratis el historial del portapapeles y los snippets que se expanden, como lanzador de código abierto (MIT) para Mac con Apple Silicon.',
    'Alfred tiene detrás una década de Workflows y funciona en Macs más antiguos; Ferrocast no intenta sustituir eso. Si lo que quieres del Powerpack es el portapapeles y los snippets, sigue leyendo. Escribo Ferrocast, así que comprueba las fuentes.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · sin cuenta' },
  limits: {
    h: 'Funciones del Powerpack de Alfred y su equivalente en Ferrocast',
    caption: 'Funciones del Powerpack de Alfred frente a lo que Ferrocast incluye gratis',
    cols: ['Función', 'Alfred', 'Ferrocast (gratis)'],
    rows: [
      ['Precio', 'La app es gratis; el Powerpack es una compra única de 34 £ (licencia individual de Alfred 5)', 'Gratis, sin plan de pago'],
      ['Historial del portapapeles', 'Powerpack; por defecto ignora las apps de contraseñas', 'Incluido; omite lo que copias desde gestores de contraseñas; tú eliges cuánto tiempo se conserva'],
      ['Snippets con autoexpansión', 'Powerpack', 'Incluidos, con marcadores como `{date}`, `{clipboard}` y `{cursor}`; importa el JSON de snippets de Raycast'],
      ['Automatización', 'Workflows (Powerpack), con miles de la comunidad', '[Script Commands](/raycast-script-commands/) compatibles con Raycast (en inglés); sin editor visual de flujos'],
      ['Comandos de shell', 'Se ejecutan en Terminal desde Alfred (Powerpack)', 'Los Script Commands se ejecutan sin abrir Terminal'],
      ['Sincronizar los ajustes entre Macs', 'Con Dropbox u otro servicio de sincronización de archivos (Powerpack)', 'Opcional con iCloud Drive (Ajustes → Advanced, desactivada por defecto). Las claves de API del Llavero se quedan en cada Mac. La exportación de copia de seguridad sigue en Ajustes'],
      ['IA', 'No consta una IA integrada en su web', 'Ask AI con tu proveedor; Ollama en tu Mac por defecto'],
      ['Conversión de archivos', 'Acciones de archivo y, a medida, con Workflows', '275 conversiones locales y 26 herramientas de archivo'],
      ['Gestión de ventanas', 'No consta en su web', 'Unos 40 comandos'],
      ['Requisitos', 'Alfred 5: macOS 10.14 o posterior', 'Apple Silicon, macOS 13 o posterior'],
      ['Código fuente', 'No consta una licencia abierta', 'MIT'],
    ],
    note: 'Columna de Alfred: [alfredapp.com/powerpack](https://www.alfredapp.com/powerpack/), [la tienda](https://www.alfredapp.com/shop/), [ayuda del portapapeles](https://www.alfredapp.com/help/features/clipboard/) y [requisitos mínimos](https://www.alfredapp.com/help/getting-started/minimum-requirements/), consultados el 28 de octubre de 2026. Columna de Ferrocast: sus [datos clave](/es/facts/) y la documentación (en inglés).',
  },
  steps: {
    h: 'Lleva tu costumbre del portapapeles y los snippets a Ferrocast',
    howToName: 'Configurar el historial del portapapeles y los snippets en Ferrocast',
    items: [
      ['Instala Ferrocast', 'Descarga el DMG o instálalo con Homebrew. No necesita cuenta.'],
      ['Elige un atajo', 'Ferrocast se abre por defecto con ⌥ Espacio; cámbialo en Ajustes si Alfred ya lo usa.'],
      ['Activa el historial del portapapeles', 'En Ajustes → Clipboard deja activado Record history y elige cuánto tiempo conservarlo.'],
      ['Recrea tus snippets', 'Abre Snippets desde el lanzador, pulsa ⌘N y escribe la palabra clave y el texto. La expansión automática necesita el permiso de Accesibilidad.'],
    ],
  },
  sections: [
    {
      h: 'Quédate en Alfred si…',
      paras: [],
      list: [
        'dependes de los Workflows, propios o de la comunidad;',
        'usas un Mac con Intel o una versión de macOS anterior a la 13;',
        'quieres Alfred 6, que según la tienda es una actualización gratis para quien compre ahora el Powerpack de Alfred 5;',
        'prefieres sincronizar los ajustes con Dropbox en vez de iCloud Drive.',
      ],
    },
    {
      h: 'Ferrocast encaja si…',
      paras: [],
      list: [
        'quieres sobre todo el historial del portapapeles y los snippets sin pagar por ellos;',
        'quieres un chat de IA con tu propio modelo, conversión de archivos que nunca sube nada o grabación de pantalla en el mismo lanzador;',
        'prefieres un software cuyo código puedes leer.',
      ],
    },
    {
      h: '¿Alfred o Raycast?',
      paras: [
        'Si dudas entre los dos, la [comparativa de Raycast, Alfred y Spotlight](/es/raycast-vs-alfred-vs-spotlight/) los pone frente a frente con fuentes oficiales, y la [alternativa a Raycast](/es/alternativa-a-raycast/) explica qué conservas si vienes de ahí. Ferrocast importa snippets de Raycast, pero no los de Alfred.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Alfred es gratis?', 'La app de Alfred es gratis. El historial del portapapeles, los snippets, los Workflows y la sincronización de ajustes forman parte del Powerpack, una compra única (34 £ por una licencia individual de Alfred 5 cuando lo comprobamos el 28 de octubre de 2026).'],
      ['¿Hay una alternativa gratis al Powerpack de Alfred?', 'Para el historial del portapapeles y los snippets, sí. Ferrocast es un lanzador gratis y de código abierto para Mac que incluye ambos. No ejecuta los Workflows de Alfred.'],
      ['¿Puede Ferrocast importar mis snippets de Alfred?', 'No directamente. Ferrocast importa snippets desde un export JSON de Raycast; los de Alfred hay que recrearlos.'],
      ['¿Ferrocast ejecuta Workflows de Alfred?', 'No. Ferrocast ejecuta Script Commands compatibles con Raycast, que son scripts normales con comentarios de metadatos.'],
      ['¿Funciona Ferrocast en Mac con Intel?', 'No. Ferrocast necesita un Mac con Apple Silicon y macOS 13 o posterior. Alfred 5 funciona desde macOS 10.14.'],
      ['¿Ferrocast está afiliado a Alfred?', 'No. Ferrocast es independiente y no está afiliado a Alfred ni a Raycast.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['English version', '/alfred-alternative/'],
      ['Alfred o Raycast: comparativa con Spotlight', '/es/raycast-vs-alfred-vs-spotlight/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Expansor de texto gratis para Mac', '/es/expansor-de-texto-mac/'],
      ['Historial del portapapeles en Mac', '/es/historial-portapapeles-mac/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
