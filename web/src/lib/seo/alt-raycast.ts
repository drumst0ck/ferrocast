/*
 * Copy for the "Raycast alternative" pages (/raycast-alternative/ and /es/alternativa-a-raycast/).
 * Source: the Notion brief "Brief: Alternativa a Raycast (ES)". Competitor facts were checked on the
 * official pages listed in `sources` on CHECKED; re-check prices every quarter and update CHECKED.
 * Rules from the brief: no speed or memory numbers for Raycast or Alfred, never say they "do not have"
 * something we only failed to find, no aggregateRating, FAQ JSON-LD text equals the visible text.
 *
 * Inline markup in strings: [text](href) for links and **text** for bold. Nothing else is parsed.
 */

import { languageFaqEs, languageFaqEn } from './interface-language';
export const CHECKED = '2026-10-27';

export type Lang = 'en' | 'es';

export interface AltCopy {
  lang: Lang;
  path: string;
  title: string;
  description: string;
  crumbs: [string, string][];
  h1: string;
  lead: string[];
  cta: { dmg: string; brew: string; note: string };
  fit: { h: string; yesH: string; yes: string[]; noH: string; no: string[] };
  keep: { h: string; items: [string, string][] };
  missing: { h: string; items: string[]; note: string };
  adds: { h: string; items: [string, string][] };
  table: { h: string; caption: string; cols: [string, string, string, string]; rows: [string, string, string, string][]; checked: string; sourcesH: string };
  price: { h: string; items: string[] };
  install: { h: string; howToName: string; steps: [string, string][]; brew: string };
  faq: { h: string; items: [string, string][] };
  close: { h: string; text: string; star: string; disclaimer: string };
  more: { h: string; links: [string, string][] };
}

/** Official pages the competitor cells come from (same list in both languages). */
export const sources: [string, string][] = [
  ['raycast.com/pricing', 'https://www.raycast.com/pricing'],
  ['manual.raycast.com/billing', 'https://manual.raycast.com/billing'],
  ['manual.raycast.com/ai/ai-credits', 'https://manual.raycast.com/ai/ai-credits'],
  ['raycast.com/new', 'https://www.raycast.com/new'],
  ['raycast.com/faq', 'https://www.raycast.com/faq'],
  ['alfredapp.com/shop', 'https://www.alfredapp.com/shop/'],
  ['alfredapp.com/powerpack', 'https://www.alfredapp.com/powerpack/'],
  ['alfredapp.com/help/features/clipboard', 'https://www.alfredapp.com/help/features/clipboard/'],
  ['alfredapp.com/help/getting-started/minimum-requirements', 'https://www.alfredapp.com/help/getting-started/minimum-requirements/'],
];

export const es: AltCopy = {
  lang: 'es',
  path: '/es/alternativa-a-raycast/',
  title: 'Alternativa gratis a Raycast para Mac | Ferrocast',
  description:
    'Ferrocast es un lanzador gratis y MIT para Mac: Script Commands de Raycast, IA local con Ollama y 275 conversiones sin subir archivos. Lo que falta, también.',
  crumbs: [['Inicio', '/es/'], ['Alternativa a Raycast', '/es/alternativa-a-raycast/']],
  h1: 'Una alternativa a Raycast gratis y de código abierto para Mac',
  lead: [
    'Ferrocast es un lanzador para Mac gratis y de código abierto (licencia MIT), escrito en Rust. Solo funciona en Mac con Apple Silicon, ocupa unos 8,5 MB y no pide cuenta. Puedes usarlo con la interfaz en español.',
    'Tus Script Commands de Raycast funcionan tal cual y tus snippets se importan desde JSON. Las extensiones que ya instalaste con Raycast también pueden ejecutarse, desactivadas por defecto y no por completo. La sincronización entre Macs es opcional por iCloud Drive. Ferrocast no tiene tienda de extensiones propia.',
  ],
  cta: { dmg: 'Descargar el DMG', brew: 'Instalar con Homebrew', note: 'Gratis · MIT · Apple Silicon · sin cuenta' },
  fit: {
    h: 'En resumen: ¿para quién es Ferrocast y para quién no?',
    yesH: 'Te encaja si…',
    yes: [
      'quieres un lanzador gratis, sin plan Pro ni suscripción;',
      'usas sobre todo búsqueda, calculadora, portapapeles, snippets, ventanas y Script Commands;',
      'prefieres la IA con tu propio modelo (Ollama en tu Mac) o con tu propia clave;',
      'conviertes archivos a menudo y no quieres subirlos a una web;',
      'quieres poder leer el código de la app que tiene acceso a tu teclado.',
    ],
    noH: 'Quédate en Raycast si…',
    no: [
      'dependes de instalar extensiones desde la Store de Raycast;',
      'necesitas la Cloud Sync de Raycast, o sincronizar con Windows o iOS;',
      'usas Search Menu Bar Items;',
      'usas la IA con contexto de la selección o de la pantalla.',
    ],
  },
  keep: {
    h: 'Qué conservas al pasar de Raycast',
    items: [
      ['Script Commands', 'Funcionan tal cual. Ferrocast lee los metadatos compatibles con Raycast y admite los modos full, compact, silent e inline. Un límite a tener en cuenta: los argumentos se pasan por línea de comandos, como en Raycast, y pueden verse con `ps` mientras el script corre. Detalles en la [guía de migración](/es/docs/getting-started/migrating-from-raycast/).'],
      ['Snippets', 'Se importan desde una exportación JSON de Raycast, en Ajustes → copia de seguridad e importación.'],
      ['Quicklinks', 'Se recrean a mano. Lleva un minuto.'],
      ['Atajos y costumbres', 'El atajo por defecto es ⌥ Espacio y se puede cambiar en Ajustes. Un consejo práctico antes de cambiar: apunta los tres o cuatro comandos que usas a diario y pruébalos primero en Ferrocast.'],
    ],
  },
  missing: {
    h: 'Qué no tiene Ferrocast (todavía)',
    items: ['tienda de extensiones propia;', 'compatibilidad completa con las extensiones de Raycast (comandos de la barra de menús, inicio de sesión OAuth, funciones de IA, selector de archivos y accesorio de enlace aún no están soportados);', 'Search Menu Bar Items;', 'contexto de IA desde la selección o la pantalla.'],
    note: 'Ferrocast solo ejecuta extensiones que ya están en `~/.config/raycast/extensions`. No las instala y no contacta con la Store de Raycast. Si necesitas una tienda sin Raycast, [Vicinae](https://github.com/vicinaehq/vicinae) y [Tinycast](https://github.com/abue-ammar/tinycast) se presentan como compatibles con esas extensiones.',
  },
  adds: {
    h: 'Lo que Ferrocast añade y un lanzador normal no',
    items: [
      ['Convertir y editar archivos sin subirlos', '275 conversiones entre 46 formatos de entrada, 26 herramientas de archivo y 7 acciones, desde una rueda de arrastrar y soltar. Todo se hace en tu Mac y el resultado queda junto al original, sin sobrescribirlo. Por ejemplo: [HEIC a JPG](/convert/heic-to-jpg/), [MOV a GIF](/convert/mov-to-gif/), [PDF a DOCX](/convert/pdf-to-docx/), [todas las conversiones](/es/files/) y [las herramientas](/es/tools/). La [comparativa con Permute y CloudConvert](/es/convertidor-de-archivos-gratis-mac/) está en español.'],
      ['Grabar la pantalla con zoom automático', 'La grabación sigue tus clics con zoom automático y la transcripción se hace en el Mac. Requiere macOS 14 o posterior y el permiso de Grabación de pantalla. [Cómo funciona](/es/docs/launcher/screen-capture/).'],
      ['Ask AI con tu propio modelo', 'Usa Ollama en tu Mac por defecto. También LM Studio y otros servidores compatibles con OpenAI, la API de Anthropic con tu clave y Claude Code. Sign in with ChatGPT funciona, pero OpenAI no lo ha autorizado para apps de terceros; la vía oficial es una API con clave. Las claves se guardan en el Llavero.'],
      ['Ventanas, portapapeles y notas', 'Unos 40 comandos para colocar ventanas. El historial del portapapeles ignora las entradas de los gestores de contraseñas. Y notas flotantes en Markdown con imágenes.'],
    ],
  },
  table: {
    h: 'Ferrocast vs Raycast vs Alfred: tabla honesta',
    caption: 'Comparativa de Ferrocast, Raycast y Alfred con datos de sus webs oficiales',
    cols: ['', 'Ferrocast', 'Raycast', 'Alfred'],
    rows: [
      ['Precio base', 'Gratis', 'Plan Free para uso personal', 'App gratis'],
      ['De pago', 'No hay plan de pago', 'Pro: 10 $/mes o 96 $/año. Pro+: 20 $/192 $. Max: 50 $/480 $', 'Powerpack, pago único (34 £ la licencia v5 individual)'],
      ['Código abierto', 'Sí, MIT', 'No consta una licencia abierta en su web', 'No consta una licencia abierta en su web'],
      ['Requisitos', 'Apple Silicon. Grabar pantalla: macOS 14 o posterior', 'Raycast 2: macOS Tahoe y Apple Silicon', 'Alfred 5: macOS 10.14 o posterior'],
      ['IA', 'Incluida, con tu proveedor. Ollama en local por defecto', 'No incluida en Free; en Pro, con créditos mensuales', 'No consta IA integrada en su web'],
      ['Portapapeles', 'Incluido; ignora gestores de contraseñas', 'Incluido en Free; historial ilimitado en planes de pago', 'Con Powerpack; ignora apps de contraseñas por defecto'],
      ['Snippets', 'Incluidos; importación desde Raycast', 'Incluidos en Free', 'Con Powerpack'],
      ['Extensiones', 'Script Commands, y extensiones ya instaladas con Raycast (parcial, desactivado por defecto); sin tienda propia', 'Store de extensiones y Script Commands', 'Workflows (Powerpack) y Alfred Gallery'],
      ['Sync entre Macs', 'Opcional, por iCloud Drive (desactivada por defecto)', 'Cloud Sync en planes de pago', 'Con Dropbox u otro servicio de archivos (Powerpack)'],
      ['Conversión de archivos local', '275 pares, 46 formatos', 'No integrada según su web', 'No integrada según su web'],
      ['Idioma de la interfaz', 'Inglés y seis más: español, alemán, francés, portugués de Brasil, japonés y chino simplificado. Por Mac, no se sincroniza', 'No se compara aquí', 'No se compara aquí'],
      ['Plataformas', 'macOS (Apple Silicon)', 'macOS, Windows (beta pública) e iOS', 'macOS'],
    ],
    checked: 'Datos de Raycast y Alfred comprobados el 27 de octubre de 2026 en sus webs oficiales. «No consta» significa que no lo encontramos en esas páginas, no que no exista. Los datos de Ferrocast salen de este proyecto y de [datos clave](/facts/).',
    sourcesH: 'Fuentes',
  },
  price: {
    h: 'Precio: ¿cuánto cuesta cada uno?',
    items: [
      '**Ferrocast:** gratis. No hay plan Pro ni funciones de pago.',
      '**Raycast:** plan Free sin IA. Pro cuesta 10 $ al mes o 96 $ al año ([raycast.com/pricing](https://www.raycast.com/pricing)).',
      '**Alfred:** la app es gratis; el Powerpack es un pago único de 34 £ por la licencia v5 individual ([alfredapp.com/shop](https://www.alfredapp.com/shop/)).',
    ],
  },
  install: {
    h: 'Cómo instalarlo y migrar en 5 minutos',
    howToName: 'Instalar Ferrocast y migrar desde Raycast',
    steps: [
      ['Descarga el DMG', 'Descarga el último Ferrocast.dmg desde GitHub.'],
      ['Arrástralo a Aplicaciones', 'Abre el DMG y arrastra Ferrocast a la carpeta Aplicaciones.'],
      ['Ábrelo', 'Si macOS lo bloquea, ve a Ajustes del Sistema → Privacidad y seguridad y elige «Abrir igualmente». La app lleva firma ad-hoc y no está notarizada.'],
      ['Concede Accesibilidad', 'Hace falta para colocar ventanas, pegar y expandir snippets.'],
      ['Copia tus Script Commands', 'Apunta Ferrocast a la carpeta de tus Script Commands; funcionan tal cual.'],
      ['Importa tus snippets', 'Exporta los snippets de Raycast a JSON e impórtalos en Ajustes.'],
    ],
    brew: 'Con Homebrew se compila en tu Mac (la primera instalación tarda unos minutos) y no aparece el aviso de «Abrir igualmente»:',
  },
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Hay una alternativa gratis y de código abierto a Raycast para Mac?', 'Sí. Ferrocast es un lanzador gratis para Mac con licencia MIT, escrito en Rust. Incluye búsqueda, calculadora, portapapeles, snippets, unos 40 comandos de ventanas y Script Commands compatibles con Raycast. No pide cuenta.'],
      ['¿Puedo usar mis extensiones de Raycast en Ferrocast?', 'En parte. Actívalo en Ajustes → Integrations → Raycast extensions. Ferrocast ejecuta entonces las extensiones que ya instalaste con Raycast, desde ~/.config/raycast/extensions. Necesita Node.js 22 o posterior (usa el tuyo, o descarga el oficial, unos 26 MB, cuando pulsas el botón). No contacta con la Store de Raycast, no está afiliado a Raycast y no tiene tienda propia. En una prueba de 40 comandos de vista de 20 extensiones populares, funcionaron 34. Aún no están soportados los comandos de la barra de menús, el inicio de sesión OAuth, las funciones de IA, el selector de archivos y el accesorio de enlace. Los Script Commands siguen funcionando tal cual.'],
      ['¿Cómo paso mis snippets y quicklinks de Raycast?', 'Los snippets se importan desde una exportación JSON de Raycast. Los quicklinks se recrean a mano.'],
      ['¿Ferrocast funciona en Mac con Intel?', 'No. Ferrocast solo funciona en Mac con Apple Silicon. Raycast 2 también exige Apple Silicon y macOS Tahoe.'],
      ['¿Ferrocast tiene IA gratis?', 'Ask AI usa Ollama en tu propio Mac por defecto, sin suscripción. También puedes conectar LM Studio, la API de OpenAI o la de Anthropic, o Claude Code. Las claves se guardan en el Llavero.'],
      ['¿Ferrocast sincroniza entre varios Macs?', 'Sí, si lo activas. Ajustes → Advanced → Sync with iCloud está desactivado por defecto y necesita iCloud Drive. Sincroniza ajustes (atajos incluidos), snippets, quicklinks, notas, el historial del portapapeles con sus imágenes, los chats de Ask AI y los temas propios. Las claves del Llavero y los ajustes de cada Mac (carpetas de scripts, el vault de Obsidian, abrir al iniciar sesión, cámara y micrófono, actualizaciones y permisos de macOS) se quedan en ese Mac. Los datos locales siguen en ~/Library/Application Support/Ferrocast como JSON.'],
      ['¿Por qué macOS pide «Abrir igualmente» al abrir Ferrocast?', 'La app lleva firma ad-hoc y no está notarizada. La primera vez hay que confirmarla en Ajustes del Sistema → Privacidad y seguridad. Con Homebrew se compila en tu Mac y no aparece ese aviso.'],
      ['¿Necesito ffmpeg?', 'No para muchas conversiones, porque Ferrocast usa primero herramientas de macOS. ffmpeg, LibreOffice, Ghostscript, cwebp y unrar son opcionales: Ferrocast solo los usa si están instalados, y las conversiones que los necesitan lo indican.'],
      ['¿Raycast es gratis?', 'Raycast tiene un plan Free para uso personal sin IA. Raycast Pro cuesta 10 $ al mes o 96 $ al año, según raycast.com/pricing (comprobado el 27 de octubre de 2026).'],
      ['¿Ferrocast está afiliado a Raycast?', 'No. Ferrocast es independiente y no está afiliado a Raycast ni a Alfred.'],
    
      languageFaqEs,
    ],
  },
  close: {
    h: 'Pruébalo',
    text: 'Hice Ferrocast porque quería un lanzador rápido que pudiera leer y cambiar, y que además convirtiera archivos sin pasar por una web. Si te ahorra tiempo, cuéntamelo.',
    star: 'Si te resulta útil, [dale una estrella en GitHub](https://github.com/drumst0ck/ferrocast).',
    disclaimer: 'Ferrocast es independiente y no está afiliado a Raycast ni a Alfred.',
  },
  more: {
    h: 'Sigue leyendo',
    links: [
      ['Guía de migración desde Raycast', '/es/docs/getting-started/migrating-from-raycast/'],
      ['Raycast vs Alfred vs Spotlight', '/es/raycast-vs-alfred-vs-spotlight/'],
      ['Alternativa gratis a Alfred sin Powerpack', '/es/alternativa-a-alfred/'],
      ['Instalación', '/es/docs/getting-started/install/'],
      ['Datos y privacidad', '/es/docs/reference/data-and-privacy/'],
      ['Preguntas frecuentes de la documentación', '/es/docs/reference/faq/'],
      ['Herramientas opcionales (ffmpeg, LibreOffice…)', '/docs/files/optional-tools/'],
      ['Datos clave (en inglés)', '/facts/'],
    ],
  },
};

export const en: AltCopy = {
  lang: 'en',
  path: '/raycast-alternative/',
  title: 'Free, open-source Raycast alternative for Mac | Ferrocast',
  description:
    'Ferrocast is a free MIT launcher for Mac that runs Raycast Script Commands, converts 275 file pairs locally and records your screen. What it lacks, too.',
  crumbs: [['Home', '/'], ['Raycast alternative', '/raycast-alternative/']],
  h1: 'A free, open-source Raycast alternative for Mac',
  lead: [
    'Ferrocast is a free, open-source (MIT) launcher for Mac, written in Rust. It is one 8.5 MB app that is also a local file converter and a screen recorder with auto zoom. Apple Silicon only, no account.',
    'Your Raycast Script Commands run as they are and your snippets import from JSON. Extensions you already installed with Raycast can run too, off by default and not in full. Sync between Macs is opt-in through iCloud Drive. Ferrocast has no extension store of its own.',
  ],
  cta: { dmg: 'Download the DMG', brew: 'Install with Homebrew', note: 'Free · MIT · Apple Silicon · no account' },
  fit: {
    h: 'In short: who Ferrocast is for, and who it is not for',
    yesH: 'Ferrocast fits if you…',
    yes: [
      'want a launcher with no Pro plan and no subscription;',
      'mostly use search, calculator, clipboard, snippets, window commands and Script Commands;',
      'want AI with your own model (Ollama on your Mac) or your own key;',
      'convert files often and would rather not upload them to a website;',
      'want to read the code of the app that sees your keyboard.',
    ],
    noH: 'Stay on Raycast if you…',
    no: [
      'rely on installing extensions from the Raycast Store;',
      'need Raycast Cloud Sync, or sync with Windows or iOS;',
      'use Search Menu Bar Items;',
      'use AI with context from your selection or screen.',
    ],
  },
  keep: {
    h: 'What you keep when you leave Raycast',
    items: [
      ['Script Commands', 'They run as they are. Ferrocast reads the Raycast-compatible metadata and supports the full, compact, silent and inline modes. One limit: arguments are passed on the command line, as in Raycast, so they show up in `ps` while the script runs. See the [migration guide](/docs/getting-started/migrating-from-raycast/).'],
      ['Snippets', 'Import them from a Raycast JSON export, in Settings → backup and import.'],
      ['Quicklinks', 'Recreate them by hand. It takes a minute.'],
      ['Shortcuts and habits', 'The default hotkey is ⌥ Space and you can change it in Settings. Before you switch, write down the three or four commands you use every day and try those first.'],
    ],
  },
  missing: {
    h: 'What Ferrocast does not have (yet)',
    items: ['an extension store of its own;', 'full Raycast extension support (menu bar commands, OAuth sign-in, AI features, the file picker and the link accessory are not supported yet);', 'Search Menu Bar Items;', 'AI context from your selection or screen.'],
    note: 'Ferrocast only runs extensions already in `~/.config/raycast/extensions`. It does not install them and it does not contact Raycast’s Store. If you need a store without Raycast, [Vicinae](https://github.com/vicinaehq/vicinae) and [Tinycast](https://github.com/abue-ammar/tinycast) describe themselves as compatible with those extensions.',
  },
  adds: {
    h: 'Three apps in one 8.5 MB launcher',
    items: [
      ['A file converter that never uploads', '275 conversions across 46 input formats, 26 file tools and 7 actions, from a drag-and-drop wheel. Everything runs on your Mac and the result lands next to the original without overwriting it. For example [HEIC to JPG](/convert/heic-to-jpg/), [MOV to GIF](/convert/mov-to-gif/), [PDF to DOCX](/convert/pdf-to-docx/) or [every conversion](/files/).'],
      ['A screen recorder with auto zoom', 'Recordings follow your clicks with auto zoom, and the transcript is made on your Mac. Needs macOS 14 or later and the Screen Recording permission. [How it works](/docs/launcher/screen-capture/).'],
      ['Ask AI with your own model', 'Ollama on your Mac by default. Also LM Studio and other OpenAI-compatible servers, the Anthropic API with your key, and Claude Code. Sign in with ChatGPT works, but OpenAI has not approved it for third-party apps; the sanctioned route is an API key. Keys are stored in the Keychain.'],
      ['Windows, clipboard and notes', 'About 40 window commands. Clipboard history skips entries from password managers. And floating Markdown notes with images.'],
    ],
  },
  table: {
    h: 'Ferrocast vs Raycast vs Alfred: an honest table',
    caption: 'Ferrocast, Raycast and Alfred compared with facts from their official websites',
    cols: ['', 'Ferrocast', 'Raycast', 'Alfred'],
    rows: [
      ['Base price', 'Free', 'Free plan for personal use', 'Free app'],
      ['Paid', 'No paid plan', 'Pro: $10/month or $96/year. Pro+: $20/$192. Max: $50/$480', 'Powerpack, one-off payment (£34 single v5 licence)'],
      ['Open source', 'Yes, MIT', 'No open licence stated on its website', 'No open licence stated on its website'],
      ['Requirements', 'Apple Silicon. Screen recording: macOS 14 or later', 'Raycast 2: macOS Tahoe and Apple Silicon', 'Alfred 5: macOS 10.14 or later'],
      ['AI', 'Included, with your provider. Local Ollama by default', 'Not in Free; in Pro, with monthly credits', 'No built-in AI stated on its website'],
      ['Clipboard', 'Included; skips password managers', 'Included in Free; unlimited history on paid plans', 'With Powerpack; skips password apps by default'],
      ['Snippets', 'Included; import from Raycast', 'Included in Free', 'With Powerpack'],
      ['Extensions', 'Script Commands, plus extensions already installed with Raycast (partial, off by default); no store of its own', 'Extension Store and Script Commands', 'Workflows (Powerpack) and Alfred Gallery'],
      ['Sync between Macs', 'Opt-in, through iCloud Drive (off by default)', 'Cloud Sync on paid plans', 'Through Dropbox or another file sync service (Powerpack)'],
      ['Local file conversion', '275 pairs, 46 formats', 'Not built in, per its website', 'Not built in, per its website'],
      ['Interface language', 'English plus six more: Spanish, German, French, Brazilian Portuguese, Japanese and Simplified Chinese. Per Mac, not synced', 'Not compared here', 'Not compared here'],
      ['Platforms', 'macOS (Apple Silicon)', 'macOS, Windows (public beta) and iOS', 'macOS'],
    ],
    checked: 'Raycast and Alfred facts checked on 27 October 2026 on their official websites. “Not stated” means we did not find it on those pages, not that it does not exist. Ferrocast facts come from this project and its [key facts](/facts/).',
    sourcesH: 'Sources',
  },
  price: {
    h: 'Price: what each one costs',
    items: [
      '**Ferrocast:** free. There is no Pro plan and no paid feature.',
      '**Raycast:** a Free plan without AI. Pro is $10 a month or $96 a year ([raycast.com/pricing](https://www.raycast.com/pricing)).',
      '**Alfred:** the app is free; the Powerpack is a one-off £34 for a single v5 licence ([alfredapp.com/shop](https://www.alfredapp.com/shop/)).',
    ],
  },
  install: {
    h: 'Install and migrate in five minutes',
    howToName: 'Install Ferrocast and migrate from Raycast',
    steps: [
      ['Download the DMG', 'Download the latest Ferrocast.dmg from GitHub.'],
      ['Drag it to Applications', 'Open the DMG and drag Ferrocast to the Applications folder.'],
      ['Open it', 'If macOS blocks it, go to System Settings → Privacy & Security and choose “Open Anyway”. The app is ad-hoc signed and not notarized.'],
      ['Grant Accessibility', 'It is needed to move windows, paste and expand snippets.'],
      ['Bring your Script Commands', 'Point Ferrocast at your Script Commands folder; they run as they are.'],
      ['Import your snippets', 'Export your Raycast snippets to JSON and import them in Settings.'],
    ],
    brew: 'With Homebrew it builds on your Mac (the first install takes a few minutes) and there is no “Open Anyway” prompt:',
  },
  faq: {
    h: 'Questions',
    items: [
      ['Is there a free, open-source Raycast alternative for Mac?', 'Yes. Ferrocast is a free Mac launcher under the MIT licence, written in Rust. It has search, a calculator, clipboard history, snippets, about 40 window commands and Raycast-compatible Script Commands. It needs no account.'],
      ['Can I use my Raycast extensions in Ferrocast?', 'Partly. Turn it on in Settings → Integrations → Raycast extensions. Ferrocast then runs extensions you already installed with Raycast, from ~/.config/raycast/extensions. It needs Node.js 22 or newer (it uses yours, or downloads the official one, about 26 MB, when you press the button). It does not contact Raycast’s Store, is not affiliated with Raycast, and has no store of its own. In a test of 40 view commands from 20 popular extensions, 34 worked. Menu bar commands, OAuth sign-in, AI features, the file picker and the link accessory are not supported yet. Script Commands still run as they are.'],
      ['How do I move my Raycast snippets and quicklinks?', 'Snippets import from a Raycast JSON export. Quicklinks are recreated by hand.'],
      ['Does Ferrocast work on Intel Macs?', 'No. Ferrocast runs only on Apple Silicon Macs. Raycast 2 also requires Apple Silicon and macOS Tahoe.'],
      ['Does Ferrocast have free AI?', 'Ask AI uses Ollama on your own Mac by default, with no subscription. You can also connect LM Studio, the OpenAI or Anthropic API, or Claude Code. Keys are stored in the Keychain.'],
      ['Does Ferrocast sync between Macs?', 'Yes, if you turn it on. Settings → Advanced → Sync with iCloud is off by default and needs iCloud Drive. It syncs settings (including hotkeys), snippets, quicklinks, notes, clipboard history with images, Ask AI chats and custom themes. Keychain API keys and per-Mac settings (script folders, the Obsidian vault, launch at login, camera and microphone, updates and macOS permissions) stay on each Mac. Local data still lives in ~/Library/Application Support/Ferrocast as JSON.'],
      ['Why does macOS ask me to “Open Anyway” for Ferrocast?', 'The app is ad-hoc signed and not notarized, so the first launch has to be confirmed in System Settings → Privacy & Security. With Homebrew it builds on your Mac and the prompt does not appear.'],
      ['Do I need ffmpeg?', 'Not for many conversions, because Ferrocast tries macOS tools first. ffmpeg, LibreOffice, Ghostscript, cwebp and unrar are optional: Ferrocast uses them only when they are installed, and the conversions that need them say so.'],
      ['Is Raycast free?', 'Raycast has a Free plan for personal use without AI. Raycast Pro costs $10 a month or $96 a year, according to raycast.com/pricing (checked on 27 October 2026).'],
      ['Is Ferrocast affiliated with Raycast?', 'No. Ferrocast is independent and not affiliated with Raycast or Alfred.'],
      languageFaqEn,
    ],
  },
  close: {
    h: 'Try it',
    text: 'I built Ferrocast because I wanted a fast launcher I could read and change, one that also converts files without a website in the middle. If it saves you time, tell me.',
    star: 'If it is useful to you, [star it on GitHub](https://github.com/drumst0ck/ferrocast).',
    disclaimer: 'Ferrocast is independent and not affiliated with Raycast or Alfred.',
  },
  more: {
    h: 'Read next',
    links: [
      ['Migrating from Raycast', '/docs/getting-started/migrating-from-raycast/'],
      ['Ferrocast vs Raycast: free vs Pro', '/ferrocast-vs-raycast/'],
      ['Install', '/docs/getting-started/install/'],
      ['Script Commands', '/raycast-script-commands/'],
      ['Data and privacy', '/docs/reference/data-and-privacy/'],
      ['Optional tools (ffmpeg, LibreOffice…)', '/docs/files/optional-tools/'],
      ['Key facts', '/facts/'],
    ],
  },
};

export { inline, plain } from './markup';
