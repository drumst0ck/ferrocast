/*
 * Copy for /text-expander-mac/ (English) and /es/expansor-de-texto-mac/ (Spanish), board idea "Free text
 * expander for Mac". Ferrocast facts come from crates/core/src/placeholders.rs (placeholders and the
 * {date}/{time} formats and offsets), crates/app/src/draw.rs (keyword prefix rule), crates/core/src/import.rs
 * (Raycast JSON import), crates/macos/src/expansion.rs (matching) and docs/launcher/snippets.md. Competitor
 * facts were read on 28 October 2026: textexpander.com/pricing, manual.raycast.com/snippets,
 * raycast.com/pricing, espanso.org and Apple Support (Text Replacement). What a source does not state is
 * written as "not stated" (never "no"). Rules: Ferrocast has no fill-in forms and says so first; sync is
 * available from 0.4.0; no rival speed or memory numbers; no import claim from TextExpander or Espanso.
 */
import type { GuideCopy } from './compress-video';

export const CHECKED = '2026-10-28';

const TE = 'https://textexpander.com/pricing';
const RAY = 'https://manual.raycast.com/snippets';
const ESP = 'https://espanso.org/';
const APPLE = 'https://support.apple.com/guide/mac-help/replace-text-punctuation-documents-mac-mh35735/mac';

export const en: GuideCopy = {
  lang: 'en',
  path: '/text-expander-mac/',
  title: 'Free Text Expander for Mac, Open Source | Ferrocast',
  description:
    'Snippets that expand as you type in any Mac app, with date, clipboard and cursor placeholders and nested snippets. Free, MIT, imports Raycast. No fill-in forms.',
  crumbs: [['Home', '/'], ['Text expander', '/text-expander-mac/']],
  h1: 'A free text expander for Mac',
  lead: [
    'Ferrocast expands snippets while you type, in any app on your Mac. They support `{date}`, `{clipboard}`, `{cursor}` and `{snippet name="…"}` to insert one snippet inside another, and Ferrocast imports a snippet export from Raycast. It is free and MIT-licensed, and I found no cap on the number of snippets in the code. **It has no fill-in forms.**',
    'If you depend on fill-in forms (fields, menus, conditional parts), team-shared snippets or scripts inside snippets, read the table first: TextExpander and Espanso cover those and Ferrocast does not. I build Ferrocast; Ferrocast is independent and not affiliated with TextExpander, Raycast or Espanso.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Ferrocast vs TextExpander, Espanso, Raycast and macOS Text Replacement',
    caption: 'Text expansion in Ferrocast, TextExpander, Espanso, Raycast and macOS compared',
    cols: ['', 'Ferrocast', 'TextExpander', 'Espanso', 'Raycast Snippets', 'Text Replacement (macOS)'],
    rows: [
      ['Price', 'Free (MIT)', 'Free plan with 5 active snippets (one swap every 2 weeks); Pro is $60 per user billed annually', 'Free, GPL-3 licence', 'Snippets are part of the free personal plan', 'Built into macOS'],
      ['Placeholders (date, clipboard, cursor)', 'Yes: date, time, clipboard, cursor, other snippets', 'Dates and times on every plan; date and time math on paid plans', 'Not stated on its homepage', 'Date, clipboard, cursor and, in v2, calculator', 'Not stated in Apple’s guide'],
      ['Snippet inside another snippet', 'Yes', 'Paid plans (listed with date math)', 'Not stated on its homepage', 'Not stated on its snippets page', 'Not stated in Apple’s guide'],
      ['Fill-in forms', '**No**', 'Yes; basic fill-ins are listed on every plan, conditional sections on paid plans', 'Yes (form support)', 'Not stated on its snippets page', 'Not stated in Apple’s guide'],
      ['Scripts in snippets', 'No', 'JavaScript, AppleScript and shell on paid plans', 'Not stated on its homepage', 'Not stated on its snippets page', 'Not stated in Apple’s guide'],
      ['Import', 'Raycast snippet JSON', 'Not stated on its pricing page', 'Not stated on its homepage', 'TextExpander, aText, Espanso and PhraseExpress exports, or JSON', 'Not stated in Apple’s guide'],
      ['Platforms', 'Mac (Apple Silicon)', 'Mac, Windows, iPhone, iPad and Chrome', 'Windows, Linux and macOS', 'Mac', 'Apple devices; Apple says it is not available in every app'],
    ],
    note: `Ferrocast: [snippets docs](/docs/launcher/snippets/) and the app’s source code. TextExpander: [${TE.replace('https://', '')}](${TE}). Espanso: [espanso.org](${ESP}). Raycast: [manual.raycast.com/snippets](${RAY}) and [raycast.com/pricing](https://www.raycast.com/pricing). macOS: [Apple Support](${APPLE}). Checked on 28 October 2026.`,
  },
  steps: {
    h: 'Create your first snippet',
    howToName: 'Create a text expansion snippet on a Mac with Ferrocast',
    items: [
      ['Install and grant permissions', 'Download the DMG or install with Homebrew. Automatic expansion needs Accessibility, and macOS may also ask for Input Monitoring.'],
      ['Open Snippets', 'Open the launcher with ⌥ Space, open Snippets and press ⌘N to create one (⌘E edits the selected snippet).'],
      ['Pick a keyword', 'Use a prefix such as `;;` or `!` so you never trigger it by accident, for example `;;sig`.'],
      ['Type it anywhere', 'Type the keyword in any app. It is replaced by the snippet text the moment the last characters you typed match it.'],
    ],
  },
  sections: [
    {
      h: 'What it does and what it does not',
      list: [
        'Does: automatic expansion in any app, pasting a snippet from the launcher, placeholders, snippets inside snippets, Raycast import, optional iCloud sync from version 0.4.0.',
        'Does not: fill-in forms (fields, menus, conditional parts), snippets shared with a team, scripts inside snippets. For those, TextExpander (paid) and Espanso (free, with forms) are the better pick.',
        'Rich text and images inside snippets are not documented, so I do not claim them.',
      ],
    },
    {
      h: 'Keyword rules',
      paras: [
        'Ferrocast expands when the last characters you typed equal a keyword, with no space needed. To avoid accidents it rejects a keyword that is a prefix of another one (or the other way round), and it rejects duplicates. A prefix like `;;` or `!` keeps normal words from triggering anything.',
      ],
    },
    {
      h: 'Placeholders: date, clipboard, cursor and nested snippets',
      list: [
        '`{date}` gives a date such as 15 Jun 2022; `{date format="yyyy-MM-dd"}` formats it, and `{date format="yyyy-MM-dd" offset="+3M -5d"}` moves it forward or back. `{time}` and `{day}` work the same way.',
        '`{clipboard}` inserts what you copied last; `{clipboard offset=1}` the one before it.',
        '`{cursor}` leaves the cursor there after expanding, so `console.log({cursor})` ends inside the parentheses.',
        '`{snippet name="sig"}` inserts another snippet by name, for example a signature inside an email template. It also resolves when you paste a snippet and in quicklinks.',
      ],
    },
    {
      h: 'Bring your snippets from Raycast',
      paras: [
        'In Raycast, use Export Snippets to save a JSON file. In Ferrocast, run the Import Snippets command from the launcher and choose that file. Entries without a name or text are skipped, exact duplicates are not added twice, and a keyword already used by another snippet is dropped from the imported one.',
        'From TextExpander, Espanso or aText there is no direct import in Ferrocast. Raycast documents that it can import from those tools, but I have not tested going through Raycast, so I do not recommend it as a route.',
      ],
    },
    {
      h: 'Permissions: why it asks for Accessibility',
      paras: [
        'Expanding while you type means watching the keys you press and typing the replacement for you, which macOS only allows with the Accessibility permission (macOS may also ask for Input Monitoring). Ferrocast’s [first run docs](/docs/getting-started/first-run/) list what each permission is for.',
      ],
    },
    {
      h: 'Between Macs',
      paras: [
        'Snippets sync through iCloud Drive when you turn on Sync with iCloud in Settings → Advanced. It is off by default and available from version 0.4.0. See the [sync docs](/docs/launcher/sync/).',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is there a free text expander for Mac?', 'Yes. Ferrocast is a free, open-source (MIT) Mac app whose snippets expand as you type in any app. Snippets support {date}, {clipboard}, {cursor} and {snippet name="…"} to insert one snippet inside another, and you can import a Raycast snippet export. It has no fill-in forms.'],
      ['Does Ferrocast have fill-in forms like TextExpander?', 'No. Ferrocast snippets support dates, clipboard, cursor position and nested snippets, but not forms with fields or menus. If you need them, TextExpander and Espanso list form support.'],
      ['How much does TextExpander cost?', 'According to its pricing page (28 October 2026), the Free plan has 5 active snippets, and Pro is $60 per user billed annually. Ferrocast is free and has no paid plan.'],
      ['Does it work in any app?', 'It expands wherever you can type, as long as Ferrocast has the Accessibility permission. Apps that block simulated typing are a risk I have not tested one by one.'],
      ['Why does it need Accessibility?', 'Automatic expansion watches the keys you type and types the replacement. macOS only allows that with Accessibility, and may also ask for Input Monitoring.'],
      ['Can I import my Raycast snippets?', 'Yes. Export them from Raycast as JSON and run Import Snippets in the Ferrocast launcher. There is no documented import from TextExpander, Espanso or aText.'],
      ['Do snippets sync between Macs?', 'Optionally. Sync with iCloud is off by default and available from version 0.4.0; snippets go through your iCloud Drive folder.'],
      ['Does it run on Intel Macs?', 'No. Ferrocast needs an Apple Silicon Mac.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Snippets docs', '/docs/launcher/snippets/'],
      ['Spanish version', '/es/expansor-de-texto-mac/'],
      ['Clipboard manager for Mac', '/clipboard-manager-mac/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['A free Alfred alternative', '/alfred-alternative/'],
      ['Key facts', '/facts/'],
    ],
  },
};

export const es: GuideCopy = {
  lang: 'es',
  path: '/es/expansor-de-texto-mac/',
  title: 'Expansor de texto gratis para Mac, de código abierto | Ferrocast',
  description:
    'Snippets que se expanden al escribir en cualquier app de Mac, con fecha, portapapeles, cursor y snippets anidados. Gratis, MIT, importa Raycast. Sin formularios',
  crumbs: [['Inicio', '/es/'], ['Expansor de texto', '/es/expansor-de-texto-mac/']],
  h1: 'Un expansor de texto gratis para Mac',
  lead: [
    'Ferrocast expande snippets mientras escribes, en cualquier app del Mac. Admiten `{date}`, `{clipboard}`, `{cursor}` y `{snippet name="…"}` para meter un snippet dentro de otro, e importa un export de snippets de Raycast. Es gratis y MIT, y en el código no he encontrado ningún límite de snippets. **No tiene formularios de relleno.**',
    'Si dependes de formularios (campos, menús, partes condicionales), de snippets compartidos en equipo o de scripts dentro de los snippets, mira antes la tabla: TextExpander y Espanso cubren eso y Ferrocast no. Escribo Ferrocast; es independiente y no está afiliado a TextExpander, Raycast ni Espanso.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · sin cuenta' },
  limits: {
    h: 'Ferrocast frente a TextExpander, Espanso, Raycast y la Sustitución de texto de macOS',
    caption: 'Expansión de texto en Ferrocast, TextExpander, Espanso, Raycast y macOS',
    cols: ['', 'Ferrocast', 'TextExpander', 'Espanso', 'Snippets de Raycast', 'Sustitución de texto (macOS)'],
    rows: [
      ['Precio', 'Gratis (MIT)', 'Plan gratis con 5 snippets activos (un cambio cada 2 semanas); Pro cuesta 60 $ por usuario facturados al año', 'Gratis, licencia GPL-3', 'Los snippets forman parte del plan personal gratis', 'Incluida en macOS'],
      ['Marcadores (fecha, portapapeles, cursor)', 'Sí: fecha, hora, portapapeles, cursor y otros snippets', 'Fechas y horas en todos los planes; cálculo de fechas y horas en los de pago', 'No consta en su portada', 'Fecha, portapapeles, cursor y, en la v2, calculadora', 'No consta en la guía de Apple'],
      ['Un snippet dentro de otro', 'Sí', 'Planes de pago (aparece junto al cálculo de fechas)', 'No consta en su portada', 'No consta en su página de snippets', 'No consta en la guía de Apple'],
      ['Formularios de relleno', '**No**', 'Sí; los básicos aparecen en todos los planes y las secciones condicionales en los de pago', 'Sí (formularios)', 'No consta en su página de snippets', 'No consta en la guía de Apple'],
      ['Scripts en los snippets', 'No', 'JavaScript, AppleScript y shell en los planes de pago', 'No consta en su portada', 'No consta en su página de snippets', 'No consta en la guía de Apple'],
      ['Importar', 'JSON de Raycast', 'No consta en su página de precios', 'No consta en su portada', 'Exportaciones de TextExpander, aText, Espanso y PhraseExpress, o JSON', 'No consta en la guía de Apple'],
      ['Plataformas', 'Mac (Apple Silicon)', 'Mac, Windows, iPhone, iPad y Chrome', 'Windows, Linux y macOS', 'Mac', 'Dispositivos Apple; Apple avisa de que no está disponible en todas las apps'],
    ],
    note: `Ferrocast: [documentación de snippets](/docs/launcher/snippets/) (en inglés) y el código de la app. TextExpander: [${TE.replace('https://', '')}](${TE}). Espanso: [espanso.org](${ESP}). Raycast: [manual.raycast.com/snippets](${RAY}) y [raycast.com/pricing](https://www.raycast.com/pricing). macOS: [guía de Apple](${APPLE}). Comprobado el 28 de octubre de 2026.`,
  },
  steps: {
    h: 'Crea tu primer snippet',
    howToName: 'Crear un snippet de expansión de texto en Mac con Ferrocast',
    items: [
      ['Instala y concede los permisos', 'Descarga el DMG o instálalo con Homebrew. La expansión automática necesita Accesibilidad, y macOS puede pedirte también Monitorización de entrada.'],
      ['Abre Snippets', 'Abre el lanzador con ⌥ Espacio, entra en Snippets y pulsa ⌘N para crear uno (⌘E edita el seleccionado).'],
      ['Elige una palabra clave', 'Usa un prefijo como `;;` o `!` para no activarlo sin querer, por ejemplo `;;firma`.'],
      ['Escríbela donde quieras', 'Escribe la palabra clave en cualquier app. Se sustituye por el texto del snippet en cuanto lo último que has escrito coincide con ella.'],
    ],
  },
  sections: [
    {
      h: 'Lo que hace y lo que no',
      list: [
        'Hace: expansión automática en cualquier app, pegar un snippet desde el lanzador, marcadores, snippets dentro de snippets, importación desde Raycast y sincronización opcional con iCloud desde la versión 0.4.0.',
        'No hace: formularios de relleno (campos, menús, partes condicionales), snippets compartidos con un equipo ni scripts dentro de los snippets. Para eso es mejor TextExpander (de pago) o Espanso (gratis, con formularios).',
        'El texto enriquecido y las imágenes dentro de los snippets no están documentados, así que no los afirmo.',
      ],
    },
    {
      h: 'Reglas de palabra clave',
      paras: [
        'Ferrocast expande cuando lo último que has escrito es igual a una palabra clave, sin necesitar un espacio. Para evitar accidentes rechaza una palabra clave que sea prefijo de otra (o al revés) y también las repetidas. Un prefijo como `;;` o `!` evita que una palabra normal dispare nada.',
      ],
    },
    {
      h: 'Marcadores: fecha, portapapeles, cursor y snippets anidados',
      list: [
        '`{date}` da una fecha como 15 Jun 2022; `{date format="yyyy-MM-dd"}` le da formato y `{date format="yyyy-MM-dd" offset="+3M -5d"}` la mueve hacia delante o atrás. `{time}` y `{day}` funcionan igual.',
        '`{clipboard}` inserta lo último que copiaste; `{clipboard offset=1}`, lo anterior.',
        '`{cursor}` deja el cursor ahí tras expandir, así que `console.log({cursor})` termina dentro de los paréntesis.',
        '`{snippet name="firma"}` inserta otro snippet por su nombre, por ejemplo una firma dentro de una plantilla de correo. También se resuelve al pegar un snippet y en los quicklinks.',
      ],
    },
    {
      h: 'Trae tus snippets desde Raycast',
      paras: [
        'En Raycast usa Export Snippets para guardar un archivo JSON. En Ferrocast ejecuta el comando Import Snippets desde el lanzador y elige ese archivo. Las entradas sin nombre o sin texto se saltan, los duplicados exactos no se añaden dos veces y la palabra clave que ya usa otro snippet se quita del importado.',
        'Desde TextExpander, Espanso o aText no hay importación directa en Ferrocast. Raycast documenta que puede importar de esas herramientas, pero no he probado pasar por Raycast, así que no lo recomiendo como ruta.',
      ],
    },
    {
      h: 'Permisos: por qué pide Accesibilidad',
      paras: [
        'Expandir mientras escribes implica vigilar las teclas que pulsas y teclear la sustitución por ti, algo que macOS solo permite con el permiso de Accesibilidad (puede pedir también Monitorización de entrada). La [documentación del primer arranque](/docs/getting-started/first-run/) (en inglés) explica para qué sirve cada permiso.',
      ],
    },
    {
      h: '¿Y entre varios Macs?',
      paras: [
        'Los snippets se sincronizan por iCloud Drive si activas Sync with iCloud en Ajustes → Advanced. Viene desactivada y está disponible desde la versión 0.4.0. Más en la [documentación de sincronización](/docs/launcher/sync/) (en inglés).',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Hay un expansor de texto gratis para Mac?', 'Sí. Ferrocast es una app gratis y de código abierto (MIT) cuyos snippets se expanden mientras escribes en cualquier app. Admiten {date}, {clipboard}, {cursor} y {snippet name="…"} para meter un snippet dentro de otro, y puedes importar un export de snippets de Raycast. No tiene formularios de relleno.'],
      ['¿Ferrocast tiene formularios de relleno como TextExpander?', 'No. Los snippets de Ferrocast admiten fecha, portapapeles, posición del cursor y snippets anidados, pero no formularios con campos ni menús. Si los necesitas, TextExpander y Espanso indican que los tienen.'],
      ['¿Cuánto cuesta TextExpander?', 'Según su página de precios (28 de octubre de 2026), el plan Free tiene 5 snippets activos y Pro cuesta 60 $ por usuario facturados al año. Ferrocast es gratis y no tiene plan de pago.'],
      ['¿Funciona en cualquier app?', 'Se expande donde puedas escribir, siempre que Ferrocast tenga el permiso de Accesibilidad. Las apps que bloquean la escritura simulada son un riesgo que no he probado una por una.'],
      ['¿Por qué necesita Accesibilidad?', 'La expansión automática vigila las teclas que pulsas y teclea la sustitución. macOS solo lo permite con Accesibilidad y puede pedir también Monitorización de entrada.'],
      ['¿Puedo importar mis snippets de Raycast?', 'Sí. Expórtalos desde Raycast como JSON y ejecuta Import Snippets en el lanzador de Ferrocast. No hay importación documentada desde TextExpander, Espanso ni aText.'],
      ['¿Se sincronizan los snippets entre Macs?', 'De forma opcional. Sync with iCloud viene desactivada y está disponible desde la versión 0.4.0; los snippets pasan por tu carpeta de iCloud Drive.'],
      ['¿Funciona en Mac con Intel?', 'No. Ferrocast necesita un Mac con Apple Silicon.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Documentación de snippets (en inglés)', '/docs/launcher/snippets/'],
      ['English version', '/text-expander-mac/'],
      ['Historial del portapapeles en Mac', '/es/historial-portapapeles-mac/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
