/*
 * Copy for /translate-selected-text-mac/ (English) and /es/traducir-texto-seleccionado-mac/ (Spanish), board
 * idea "Traducir texto seleccionado en cualquier app de Mac y reemplazarlo en su sitio, con IA local".
 * Ferrocast facts come from the code: crates/core/src/translate.rs (DEFAULT_LANGUAGES, MAX_LANGUAGES = 12,
 * MAX_CHARS = 20,000, browser_for() lists Safari, Chrome, Chrome Canary, Brave, Edge, Arc, Chromium, Vivaldi and
 * Opera and returns None for Firefox, delivery() for Auto / window / paste / copy, JS_PERMISSION_HINT),
 * crates/core/src/models.rs (wheel_translate_key "Command", TranslateOutput::Auto default),
 * crates/macos/src/drag.rs (the dragged text is read from the drag pasteboard), crates/app/src/app.rs
 * (translate_log_line records language, engine, character count and time, never the text),
 * crates/macos/swift/translate (Apple Translation, macOS 26+) and docs/launcher/translate.md and ask-ai.md.
 * Apple facts were read on CHECKED on support.apple.com (Mac User Guide, "Translate text on Mac"): Control-click
 * the selection, Translate, Replace with Translation; languages can be downloaded for offline use; On-Device
 * Mode. Dedicated translation apps are not compared (their pages were not opened). Rules: the page says first
 * that macOS already translates and replaces text; the text goes to the provider you chose (only Ollama, LM
 * Studio and Apple keep it on the Mac); no quality, speed or memory comparison.
 */
import type { GuideCopy } from './compress-video';
import { languageFaqEs, languageFaqEn } from './interface-language';

export const CHECKED = '2026-10-28';

const APPLE = 'https://support.apple.com/guide/mac-help/translate-text-on-mac-mchldd8b3c15/mac';
const APPLE_ES = 'https://support.apple.com/es-es/guide/mac-help/mchldd8b3c15/mac';

export const en: GuideCopy = {
  lang: 'en',
  path: '/translate-selected-text-mac/',
  title: 'Translate Selected Text on Mac and Replace It In Place | Ferrocast',
  description:
    'Select text in any Mac app, drag it, tap ⌘ and drop it on a language: Ferrocast translates it with your AI and replaces it. Ollama keeps it on your Mac.',
  crumbs: [['Home', '/'], ['Translate selected text', '/translate-selected-text-mac/']],
  h1: 'Translate selected text on a Mac and replace it in place',
  lead: [
    'Select some text in any app, start dragging it, tap ⌘ and drop it on a language. Ferrocast sends the text to the AI provider you chose and puts the translation back where the text was: in text fields and in Safari and Chromium pages. Where it cannot replace the text, it shows the translation in a window and copies it. With Ollama or Apple’s on-device translation the text never leaves your Mac. Free and open source (MIT).',
    'macOS already translates text on its own: Control-click the selection, choose Translate, then Replace with Translation. Ferrocast is for when you want a different route: one drag gesture, your own model or provider, and replacement inside web pages. I make Ferrocast, so the table links Apple’s page.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Ferrocast and macOS Translate, side by side',
    caption: 'Translating selected text and replacing it on a Mac with Ferrocast and with Translate in macOS',
    cols: ['', 'Ferrocast', 'Translate in macOS'],
    rows: [
      ['Gesture', 'Select, drag the selection, tap ⌘ (the default; changeable in Settings → Translate), drop on a language', 'Select, Control-click, choose Translate, then Replace with Translation'],
      ['Replace in place', 'Text fields (through Accessibility) and Safari and Chromium pages; otherwise a window with the translation, also copied', 'Apple describes Replace with Translation when you are writing on your Mac'],
      ['Who translates', 'Your AI provider (Ollama, LM Studio or any OpenAI-compatible server, the Anthropic API, ChatGPT sign-in, Claude Code), or Apple Translation on macOS 26', 'Apple Translation; you can download languages for offline use, and On-Device Mode processes translations on the Mac'],
      ['Where the text goes', 'Only to the provider you chose; with Ollama, LM Studio or the Apple engine it stays on your Mac', 'Apple describes offline languages and On-Device Mode; offline translations may be less accurate than those processed on Apple’s servers'],
      ['Languages', 'Up to 12 on the wheel, in your order; default: English, Spanish, French, German, Portuguese, Italian, Japanese, Chinese', 'The languages Apple supports, chosen in pop-up menus'],
      ['Web pages', 'Safari and Chromium browsers after you turn on “Allow JavaScript from Apple Events”; Firefox is not supported', 'Apple says you can select text on webpages to translate it'],
      ['Price', 'Free (MIT); a cloud provider bills you for your own key', 'Part of macOS'],
    ],
    note: `Apple: [Translate text on Mac](${APPLE}), read on 28 October 2026. Ferrocast: [Translate selected text docs](/docs/launcher/translate/) and the app’s source code. Dedicated translation apps are not in this table: I did not open their pages.`,
  },
  steps: {
    h: 'Translate selected text with Ferrocast',
    howToName: 'Translate selected text on a Mac and replace it in place with Ferrocast',
    items: [
      ['Install Ferrocast and pick an engine', 'Download the DMG or install it with Homebrew. In Settings → Translate → Engine, leave “Same as chat” (Ollama on your Mac by default) or pick a provider, or Apple on macOS 26. For Ollama, install it and pull a model first; Settings → AI has a Test connection button.'],
      ['Select and drag the text', 'Select text in any app and start dragging it. Keep the mouse button down.'],
      ['Tap ⌘', 'A purple wheel with your languages opens under the pointer. Escape cancels.'],
      ['Drop it on a language', 'Release over a language. A glow shows the text being translated and a moment later the translation replaces it.'],
      ['For web pages, allow JavaScript from Apple Events once', 'Turn on “Allow JavaScript from Apple Events” in your browser’s Developer menu (in Safari, after showing the developer features). macOS then asks whether Ferrocast may control the browser: allow it.'],
    ],
  },
  sections: [
    {
      h: 'Where the translation ends up',
      paras: [
        'Settings → Translate → After translating decides. The default, Auto, replaces the selection where the app allows it and shows a window otherwise. Window always shows the translation, with Copy and “Paste into app”. Paste ⌘V copies the translation and presses ⌘V, which only helps in editable fields. Copy only puts it on the clipboard.',
        'With Auto, text fields (Notes, Mail, TextEdit and most native apps) are replaced through Accessibility, with ⌘V as a fallback if the app refuses. In Safari and Chromium browsers (Chrome, Brave, Edge, Arc, Vivaldi, Opera) the selected text is replaced inside the page until you reload it. Anywhere else, or if anything fails, the window opens with the translation. If you switch to another app while it translates, Ferrocast never writes into that app.',
      ],
    },
    {
      h: 'Keep it on your Mac',
      paras: [
        'The selected text goes only to the provider you chose. Ollama and LM Studio run on your Mac, and so does the Apple engine (macOS 26 or later; the first translation after launch can take a few seconds while the model loads, and Apple decides whether a language must be downloaded first). OpenAI-compatible cloud servers, the Anthropic API, ChatGPT sign-in and Claude Code send the text to that provider. Ferrocast’s log records the language, the engine, the number of characters and the time, never the text.',
      ],
    },
    {
      h: 'Limits',
      list: [
        'Firefox is not supported for in-page replacement.',
        'Pages such as `chrome://` or `brave://` and the browser extension stores do not accept scripts, and text inside frames (some editors and embedded comments) is not replaced: the window opens instead.',
        'Pages that redraw their text (some web apps) can undo the replacement.',
        'Selections over 20,000 characters are refused.',
        'The gesture needs an app that lets you drag selected text.',
        'The quality of the translation depends on the model you choose. I have not compared it with Apple’s.',
        'Apple Translation as the engine needs macOS 26 or later.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['How do I translate selected text on a Mac and replace it?', 'With Ferrocast: select the text, start dragging it, tap ⌘ and drop it on a language. The translation replaces the selection in text fields and in Safari and Chromium pages; elsewhere it opens in a window and is copied. With macOS alone: Control-click the selection, choose Translate, then Replace with Translation.'],
      ['Does macOS already translate selected text?', 'Yes. Apple’s Mac User Guide says to select the text, Control-click it and choose Translate, and to click Replace with Translation to replace what you wrote. Ferrocast adds a drag gesture and your choice of AI provider.'],
      ['Can I translate with Ollama so nothing leaves my Mac?', 'Yes. Ferrocast’s translation uses the same providers as Ask AI, and Ollama on your Mac is the default. The Apple engine, on macOS 26 or later, also translates on your Mac.'],
      ['Does it work in the browser?', 'In Safari and Chromium browsers such as Chrome, Brave, Edge, Arc, Vivaldi and Opera, once you turn on “Allow JavaScript from Apple Events”. Firefox is not supported.'],
      ['Where does the translated text go?', 'Auto replaces the selection where the app allows it and opens a window otherwise; when it opens the window the translation is also copied. You can switch to Window, Paste ⌘V or Copy only in Settings → Translate.'],
      ['Which languages can I use?', 'Up to 12 on the wheel, in the order you list them in Settings → Translate. The defaults are English, Spanish, French, German, Portuguese, Italian, Japanese and Chinese.'],
      ['Is there a size limit?', 'Yes. Selections over 20,000 characters are refused.'],
      ['Is it free?', 'Ferrocast is free and open source (MIT). A local model costs nothing; a cloud provider bills you directly for your own key.'],
    
      languageFaqEn,
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Translate selected text docs', '/docs/launcher/translate/'],
      ['Spanish version', '/es/traducir-texto-seleccionado-mac/'],
      ['Mac launcher with local AI', '/local-ai-launcher/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['A free Alfred alternative', '/alfred-alternative/'],
      ['Key facts', '/facts/'],
    ],
  },
};

export const es: GuideCopy = {
  lang: 'es',
  path: '/es/traducir-texto-seleccionado-mac/',
  title: 'Traducir texto seleccionado en Mac y reemplazarlo | Ferrocast',
  description:
    'Arrastra el texto seleccionado en una app de Mac, toca ⌘ y suéltalo sobre un idioma: Ferrocast lo traduce con tu IA y lo reemplaza. Con Ollama, en local.',
  crumbs: [['Inicio', '/es/'], ['Traducir texto seleccionado', '/es/traducir-texto-seleccionado-mac/']],
  h1: 'Traducir el texto seleccionado en Mac y reemplazarlo en su sitio',
  lead: [
    'Selecciona texto en cualquier app, empieza a arrastrarlo, toca ⌘ y suéltalo sobre un idioma. Ferrocast envía el texto al proveedor de IA que hayas elegido y deja la traducción donde estaba el texto: en campos de texto y en páginas de Safari y de navegadores Chromium. Si no puede reemplazarlo, muestra la traducción en una ventana y la copia. Con Ollama o con la traducción de Apple en el propio Mac, el texto no sale de él. Gratis y de código abierto (MIT). Puedes usarlo con la interfaz en español.',
    'macOS ya traduce texto por sí solo: haz clic con la tecla Control pulsada en la selección, elige Traducir y luego «Reemplazar por la traducción». Ferrocast sirve cuando quieres otro recorrido: un único gesto de arrastre, tu propio modelo o proveedor y el reemplazo dentro de páginas web. Escribo Ferrocast, así que la tabla enlaza la página de Apple.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · sin cuenta' },
  limits: {
    h: 'Ferrocast y Traducir de macOS, frente a frente',
    caption: 'Traducir el texto seleccionado y reemplazarlo en Mac con Ferrocast y con Traducir de macOS',
    cols: ['', 'Ferrocast', 'Traducir en macOS'],
    rows: [
      ['Gesto', 'Seleccionar, arrastrar la selección, tocar ⌘ (por defecto; se cambia en Ajustes → Translate) y soltar sobre un idioma', 'Seleccionar, clic con Control pulsada, elegir Traducir y luego «Reemplazar por la traducción»'],
      ['Reemplazar en su sitio', 'Campos de texto (con Accesibilidad) y páginas de Safari y de navegadores Chromium; en otros casos, una ventana con la traducción, que también se copia', 'Apple describe «Reemplazar por la traducción» cuando escribes en el Mac'],
      ['Quién traduce', 'Tu proveedor de IA (Ollama, LM Studio o cualquier servidor compatible con OpenAI, la API de Anthropic, el inicio de sesión de ChatGPT, Claude Code) o Apple Translation en macOS 26', 'Apple Translation; puedes descargar idiomas para traducir sin conexión y «Traducción local» procesa las traducciones en el Mac'],
      ['Adónde va el texto', 'Solo al proveedor que elijas; con Ollama, LM Studio o el motor de Apple se queda en tu Mac', 'Apple describe los idiomas sin conexión y «Traducción local»; las traducciones sin conexión pueden ser menos precisas que las procesadas en los servidores de Apple'],
      ['Idiomas', 'Hasta 12 en la rueda, en el orden que quieras; por defecto: inglés, español, francés, alemán, portugués, italiano, japonés y chino', 'Los idiomas que admite Apple, elegidos en menús desplegables'],
      ['Páginas web', 'Safari y navegadores Chromium, tras activar «Allow JavaScript from Apple Events»; Firefox no se admite', 'Apple indica que puedes seleccionar texto de páginas web para traducirlo'],
      ['Precio', 'Gratis (MIT); un proveedor en la nube te factura por tu propia clave', 'Incluido en macOS'],
    ],
    note: `Apple: [Traducir texto en el Mac](${APPLE_ES}), consultada el 28 de octubre de 2026. Ferrocast: [documentación de Translate](/es/docs/launcher/translate/) y el código de la app. No incluyo apps de traducción dedicadas: no he abierto sus páginas.`,
  },
  steps: {
    h: 'Traducir texto seleccionado con Ferrocast',
    howToName: 'Traducir el texto seleccionado en Mac y reemplazarlo en su sitio con Ferrocast',
    items: [
      ['Instala Ferrocast y elige un motor', 'Descarga el DMG o instálalo con Homebrew. En Ajustes → Translate → Engine deja «Same as chat» (Ollama en tu Mac por defecto), elige un proveedor o Apple en macOS 26. Con Ollama, instálalo y descarga antes un modelo; Ajustes → AI tiene un botón Test connection.'],
      ['Selecciona y arrastra el texto', 'Selecciona texto en cualquier app y empieza a arrastrarlo. Mantén pulsado el botón del ratón.'],
      ['Toca ⌘', 'Se abre bajo el puntero una rueda morada con tus idiomas. Escape cancela.'],
      ['Suéltalo sobre un idioma', 'Suelta sobre un idioma. Un brillo marca el texto que se está traduciendo y, un momento después, la traducción lo reemplaza.'],
      ['En páginas web, permite JavaScript desde Eventos de Apple una vez', 'Activa «Permitir JavaScript desde Eventos de Apple» en el menú de desarrollo de tu navegador (en Safari, tras mostrar las funciones para desarrolladores). macOS preguntará si Ferrocast puede controlar el navegador: permítelo.'],
    ],
  },
  sections: [
    {
      h: 'Dónde acaba la traducción',
      paras: [
        'Lo decide Ajustes → Translate → After translating. Auto, el valor por defecto, reemplaza la selección donde la app lo permite y, si no, muestra una ventana. Window muestra siempre la traducción, con Copy y «Paste into app». Paste ⌘V copia la traducción y pulsa ⌘V, algo que solo sirve en campos editables. Copy only la deja en el portapapeles.',
        'Con Auto, los campos de texto (Notas, Mail, TextEdit y la mayoría de apps nativas) se reemplazan mediante Accesibilidad, con ⌘V como alternativa si la app lo rechaza. En Safari y en navegadores Chromium (Chrome, Brave, Edge, Arc, Vivaldi, Opera) el texto seleccionado se reemplaza dentro de la página hasta que la recargues. En cualquier otro sitio, o si algo falla, se abre la ventana con la traducción. Si cambias a otra app mientras traduce, Ferrocast nunca escribe en esa app.',
      ],
    },
    {
      h: 'Que no salga de tu Mac',
      paras: [
        'El texto seleccionado va solo al proveedor que elijas. Ollama y LM Studio se ejecutan en tu Mac, y el motor de Apple también (macOS 26 o posterior; la primera traducción tras abrir la app puede tardar unos segundos mientras carga el modelo, y es Apple quien decide si hay que descargar antes un idioma). Los servidores en la nube compatibles con OpenAI, la API de Anthropic, el inicio de sesión de ChatGPT y Claude Code envían el texto a ese proveedor. El registro de Ferrocast guarda el idioma, el motor, el número de caracteres y el tiempo, nunca el texto.',
      ],
    },
    {
      h: 'Límites',
      list: [
        'Firefox no se admite para el reemplazo dentro de la página.',
        'Páginas como `chrome://` o `brave://` y las tiendas de extensiones no aceptan scripts, y el texto dentro de marcos (algunos editores y comentarios incrustados) no se reemplaza: se abre la ventana.',
        'Las páginas que vuelven a dibujar su texto (algunas apps web) pueden deshacer el reemplazo.',
        'Se rechazan las selecciones de más de 20 000 caracteres.',
        'El gesto necesita una app que te deje arrastrar el texto seleccionado.',
        'La calidad de la traducción depende del modelo que elijas. No la he comparado con la de Apple.',
        'Apple Translation como motor necesita macOS 26 o posterior.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Cómo traduzco el texto seleccionado en Mac y lo reemplazo?', 'Con Ferrocast: selecciona el texto, empieza a arrastrarlo, toca ⌘ y suéltalo sobre un idioma. La traducción reemplaza la selección en campos de texto y en páginas de Safari y de navegadores Chromium; en otros sitios se abre en una ventana y se copia. Solo con macOS: haz clic con Control pulsada en la selección, elige Traducir y luego «Reemplazar por la traducción».'],
      ['¿macOS ya traduce el texto seleccionado?', 'Sí. El manual de uso del Mac de Apple indica que selecciones el texto, hagas clic con Control pulsada y elijas Traducir, y que hagas clic en «Reemplazar por la traducción» para sustituir lo que has escrito. Ferrocast añade un gesto de arrastre y la posibilidad de elegir tu proveedor de IA.'],
      ['¿Puedo traducir con Ollama para que nada salga de mi Mac?', 'Sí. La traducción de Ferrocast usa los mismos proveedores que Ask AI, y Ollama en tu Mac es el predeterminado. El motor de Apple, en macOS 26 o posterior, también traduce en tu Mac.'],
      ['¿Funciona en el navegador?', 'En Safari y en navegadores Chromium como Chrome, Brave, Edge, Arc, Vivaldi y Opera, una vez activado «Allow JavaScript from Apple Events». Firefox no se admite.'],
      ['¿Adónde va el texto traducido?', 'Auto reemplaza la selección donde la app lo permite y abre una ventana en caso contrario; cuando abre la ventana, la traducción también se copia. Puedes pasar a Window, Paste ⌘V o Copy only en Ajustes → Translate.'],
      ['¿Qué idiomas puedo usar?', 'Hasta 12 en la rueda, en el orden en que los pongas en Ajustes → Translate. Por defecto son inglés, español, francés, alemán, portugués, italiano, japonés y chino.'],
      ['¿Hay un límite de tamaño?', 'Sí. Se rechazan las selecciones de más de 20 000 caracteres.'],
      ['¿Es gratis?', 'Ferrocast es gratis y de código abierto (MIT). Un modelo local no cuesta nada; un proveedor en la nube te factura directamente por tu propia clave.'],
    
      languageFaqEs,
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Documentación de Translate', '/es/docs/launcher/translate/'],
      ['English version', '/translate-selected-text-mac/'],
      ['Lanzador con IA local en Mac', '/es/ia-local-mac/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Alternativa gratis a Alfred', '/es/alternativa-a-alfred/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
