/*
 * Copy for /es/ia-local-mac/ (Spanish, sister of /local-ai-launcher/). Ferrocast facts come from
 * docs/launcher/ask-ai.md, translate.md and notes.md. The Raycast line comes from raycast.com/pricing,
 * read on 2026-10-28 ("Raycast AI is included in paid Pro plans"; "AI isn't available on the Free
 * plan"). Rules: say plainly which providers send text off the Mac, keep the ChatGPT sign-in caveat, never
 * claim AI context from the screen or selection, no subjective comparison with Raycast AI.
 */
import type { GuideCopy } from './compress-video';

export const page: GuideCopy = {
  lang: 'es',
  path: '/es/ia-local-mac/',
  title: 'Lanzador con IA local en Mac: Ollama, LM Studio, Claude Code | Ferrocast',
  description:
    'Ferrocast es un lanzador gratis para Mac cuyo chat de IA usa Ollama en tu Mac por defecto, o LM Studio, tu propia clave o Claude Code. Sin suscripción.',
  crumbs: [['Inicio', '/es/'], ['IA local en Mac', '/es/ia-local-mac/']],
  h1: 'Un lanzador para Mac con IA local, y tus propias claves cuando quieras',
  lead: [
    'Ask AI en Ferrocast es un chat con respuestas en streaming y historial de conversaciones. De serie habla con **Ollama en tu propio Mac**, así que nada sale de él. Desde un menú del chat puedes cambiar a LM Studio, a cualquier servidor compatible con OpenAI, a la API de Anthropic con tu clave o a Claude Code con tu suscripción de Claude.',
    'Ferrocast es gratis y de código abierto (MIT) y no hay un plan de IA que comprar. En Raycast la IA forma parte de los planes de pago y no está disponible en el plan Free ([raycast.com/pricing](https://www.raycast.com/pricing), comprobado el 28 de octubre de 2026). Escribo Ferrocast; no está afiliado a Raycast, Ollama ni LM Studio.',
  ],
  cta: { dmg: 'Descargar Ferrocast', note: 'Gratis · MIT · Apple Silicon · sin cuenta' },
  limits: {
    h: 'Proveedores y adónde va tu texto',
    caption: 'Proveedores de IA que admite Ferrocast y si el texto sale del Mac',
    cols: ['Proveedor', 'Cómo lo usa Ferrocast', '¿Sale de tu Mac?'],
    rows: [
      ['Ollama (por defecto)', 'Un servidor compatible con OpenAI que corre en tu Mac; ya viene como preajuste en Ajustes → AI.', 'No'],
      ['LM Studio', 'Igual, con su propio preajuste.', 'No'],
      ['OpenAI y otros servidores compatibles con OpenAI', 'Tu clave de API, guardada en el Llavero; los modelos se listan desde el servidor.', 'Sí, al proveedor'],
      ['API de Anthropic', 'Tu propia clave, guardada en el Llavero.', 'Sí, a Anthropic'],
      ['Claude Code', 'La CLI oficial con tu suscripción de Claude, sin herramientas, con una sesión en caliente mientras el chat está abierto.', 'Sí, a Anthropic'],
      ['Iniciar sesión con ChatGPT', 'El flujo OAuth que usa la CLI Codex, para que un plan de ChatGPT responda sin clave de API. OpenAI no ha aprobado su uso por terceros.', 'Sí, a OpenAI'],
    ],
    note: 'Según la [documentación de Ask AI](/docs/launcher/ask-ai/) (en inglés). **Test connection** en Ajustes → AI pide al servidor su lista de modelos.',
  },
  steps: {
    h: 'Configura la IA local en unos minutos',
    howToName: 'Usar un modelo de IA local desde el lanzador de Ferrocast con Ollama',
    items: [
      ['Instala Ollama', 'Instala Ollama desde ollama.com y descarga un modelo con ollama pull seguido del nombre del modelo.'],
      ['Instala Ferrocast', 'Descarga el DMG o instálalo con Homebrew.'],
      ['Comprueba la conexión', 'En Ajustes → AI deja el preajuste de Ollama y pulsa Test connection; elige un modelo de la lista.'],
      ['Pregunta', 'Abre Ask AI desde el lanzador y escribe. Cambia de proveedor o de modelo desde la pastilla del chat.'],
    ],
  },
  sections: [
    {
      h: 'Dónde más se usa la IA',
      list: [
        '**Traducir el texto seleccionado:** arrastra una selección, toca ⌘ y suéltala sobre un idioma; la traducción sustituye al texto donde es posible. Mira [Translate](/docs/launcher/translate/) (en inglés).',
        '**Notas:** comandos de IA en la ventana flotante de notas. Mira [Notes](/docs/launcher/notes/) (en inglés).',
        'Cada función puede usar su propio proveedor y modelo.',
      ],
    },
    {
      h: 'Límites',
      list: [
        'Ask AI no lee tu pantalla ni la selección actual: tú pegas aquello sobre lo que quieres preguntar.',
        'Iniciar sesión con ChatGPT es una zona gris de los términos de OpenAI; una clave de API es la vía reglamentaria.',
        'Las suscripciones de ChatGPT y de Claude no incluyen acceso a la API, por eso se ofrecen mediante el inicio de sesión y Claude Code.',
        'Los modelos locales necesitan la memoria y el espacio en disco que pidan Ollama o LM Studio.',
        'Solo Mac con Apple Silicon.',
      ],
    },
  ],
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Hay un lanzador para Mac que funcione con Ollama?', 'Sí. Ask AI de Ferrocast habla por defecto con Ollama en tu Mac, a través de su servidor compatible con OpenAI, y mantiene el chat en tu Mac.'],
      ['¿Puedo usar LM Studio en vez de Ollama?', 'Sí. Ajustes → AI tiene un preajuste de LM Studio; Ferrocast lista los modelos que indica el servidor.'],
      ['¿Puedo usar mi suscripción de Claude?', 'Sí, a través de Claude Code, la CLI oficial, si la tienes instalada. Ferrocast la ejecuta sin herramientas y mantiene una sesión en caliente mientras el chat está abierto.'],
      ['¿Necesito una suscripción para la IA de Ferrocast?', 'No. Ferrocast no tiene plan de pago. Un modelo local es gratis; los proveedores en la nube te facturan directamente por tu propia clave.'],
      ['¿Están seguras mis claves de API?', 'Las claves se guardan en el Llavero de macOS, no en los archivos de ajustes de Ferrocast.'],
    ],
  },
  more: {
    h: 'Relacionado',
    links: [
      ['Documentación de Ask AI (en inglés)', '/docs/launcher/ask-ai/'],
      ['English version', '/local-ai-launcher/'],
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Raycast vs Alfred vs Spotlight', '/es/raycast-vs-alfred-vs-spotlight/'],
      ['Datos clave', '/es/facts/'],
    ],
  },
};
