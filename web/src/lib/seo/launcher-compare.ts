/*
 * Copy for /es/raycast-vs-alfred-vs-spotlight/ (Spanish only, per the Notion board idea).
 * Source: the Notion brief "Brief: Raycast vs Alfred vs Spotlight (ES)". Spotlight facts come from Apple
 * Support pages; Raycast and Alfred facts are the ones checked for /es/alternativa-a-raycast/ (same
 * date and sources, see alt-raycast.ts). Rules: no subjective scores, no rival speed or memory numbers,
 * never "no tiene" for something we only failed to find; say that Ferrocast is made by the site author.
 */
import { sources as rivalSources } from './alt-raycast';

export const CHECKED = '2026-10-27';

export const page = {
  path: '/es/raycast-vs-alfred-vs-spotlight/',
  title: 'Raycast vs Alfred vs Spotlight en macOS Tahoe: cuál elegir | Ferrocast',
  description:
    'Comparativa en español de Spotlight (macOS Tahoe), Raycast, Alfred y Ferrocast: precio, portapapeles, snippets, ventanas, IA y archivos, con fuentes oficiales.',
  crumbs: [['Inicio', '/es/'], ['Raycast vs Alfred vs Spotlight', '/es/raycast-vs-alfred-vs-spotlight/']] as [string, string][],
  h1: 'Raycast, Alfred o Spotlight: qué lanzador elegir en tu Mac',
  lead: 'Desde macOS Tahoe, Spotlight tiene historial del portapapeles y acciones, así que para mucha gente ya basta. Raycast y Alfred siguen yendo más lejos en snippets, extensiones y automatización. Esta comparativa recoge solo lo que publican sus webs oficiales. Aviso: escribo Ferrocast, la cuarta columna, y lo incluyo con lo que tiene y lo que le falta.',
  short: {
    h: 'La respuesta corta',
    items: [
      '**Spotlight** basta si quieres abrir apps y archivos, consultar el portapapeles y lanzar acciones del sistema, sin instalar nada.',
      '**Alfred** encaja si quieres automatizar con Workflows y prefieres un pago único (Powerpack) a una suscripción.',
      '**Raycast** encaja si dependes de extensiones de su Store, de la sincronización entre Macs o de su IA de pago.',
      '**Ferrocast** encaja si quieres un lanzador gratis y de código abierto que además convierta archivos sin subirlos y grabe la pantalla. La sincronización con iCloud es opcional y está desactivada por defecto. No tiene tienda de extensiones propia, y puede ejecutar extensiones que ya tengas instaladas con Raycast, con límites.',
    ],
  },
  table: {
    h: 'Comparativa con fuentes',
    caption: 'Spotlight, Raycast, Alfred y Ferrocast comparados con datos de sus webs oficiales',
    cols: ['', 'Spotlight (macOS Tahoe)', 'Raycast', 'Alfred', 'Ferrocast'],
    rows: [
      ['Precio', 'Incluido en macOS', 'Free para uso personal; Pro desde 10 $/mes o 96 $/año', 'App gratis; Powerpack de pago único (34 £, licencia v5 individual)', 'Gratis, sin plan de pago'],
      ['Portapapeles', 'Sí, con ⌘4. Apple avisa de que puede aparecer información sensible', 'Incluido en Free; historial ilimitado en planes de pago', 'Con Powerpack; ignora apps de contraseñas por defecto', 'Incluido; salta lo copiado desde gestores de contraseñas'],
      ['Snippets', 'No consta en la guía de Spotlight; macOS tiene Sustituciones de texto en Ajustes → Teclado', 'Incluidos en Free', 'Con Powerpack', 'Incluidos; importación desde Raycast'],
      ['Ventanas', 'No consta en la guía de Spotlight; macOS coloca ventanas con el botón verde, el menú Ventana y Fn-Control-flechas', 'Incluido en Free; comandos personalizados en Pro', 'No consta en su web', 'Unos 40 comandos'],
      ['Acciones y extensiones', 'Acciones del sistema y de apps, quick keys y atajos de la app Atajos', 'Store de extensiones y Script Commands', 'Workflows (Powerpack) y Alfred Gallery', 'Script Commands, y extensiones ya instaladas con Raycast (parcial, desactivado por defecto); sin tienda propia'],
      ['IA', 'Más acciones con Apple Intelligence', 'No incluida en Free; en Pro, con créditos mensuales', 'No consta IA integrada en su web', 'Con tu proveedor; Ollama en local por defecto'],
      ['Archivos', 'En el Finder, la acción rápida Convertir imagen (formato y tamaño)', 'No integrada según su web', 'No integrada según su web', '275 conversiones y 26 herramientas, en local'],
      ['Sync entre Macs', 'No consta para el portapapeles en la guía de Apple', 'Cloud Sync en planes de pago', 'Con Dropbox u otro servicio de archivos (Powerpack)', 'Opcional, por iCloud Drive (desactivada por defecto). No es la Cloud Sync de Raycast'],
      ['Código abierto', 'No, es parte de macOS', 'No consta una licencia abierta en su web', 'No consta una licencia abierta en su web', 'Sí, MIT'],
      ['Requisitos', 'macOS Tahoe para portapapeles y acciones', 'Raycast 2: macOS Tahoe y Apple Silicon', 'Alfred 5: macOS 10.14 o posterior', 'Apple Silicon'],
    ] as [string, string, string, string, string][],
    checked: 'Comprobado el 27 de octubre de 2026 en las webs oficiales de Apple, Raycast y Alfred. «No consta» significa que no lo encontramos en esas páginas, no que no exista.',
  },
  apps: {
    h: 'Cada uno, en detalle',
    items: [
      ['Spotlight en macOS Tahoe', 'Se abre con ⌘ Espacio. Además de apps y archivos, tiene una sección de acciones (⌘3) en la que puedes asignar quick keys, por ejemplo «ft» para iniciar una llamada de FaceTime, y el historial del portapapeles (⌘4), que se borra con Clear History. Lo que la guía de Apple no recoge en Spotlight: snippets (para eso están las Sustituciones de texto de macOS), comandos de ventanas desde el buscador y extensiones de terceros más allá de las acciones de las apps y la app Atajos.'],
      ['Raycast', 'El plan Free incluye portapapeles, snippets y ventanas. La IA, el historial ilimitado del portapapeles y Cloud Sync son de pago (Pro desde 10 $ al mes). Su punto fuerte es la Store de extensiones. Raycast 2 pide macOS Tahoe y Apple Silicon y también funciona en Windows (beta pública) e iOS.'],
      ['Alfred', 'La app es gratis; el portapapeles, los snippets y los Workflows forman parte del Powerpack, un pago único de 34 £. El portapapeles ignora por defecto las apps de contraseñas. Alfred 5 funciona desde macOS 10.14, así que es la opción para Macs con Intel o sistemas antiguos.'],
      ['Ferrocast', 'Gratis y de código abierto (MIT), escrito en Rust, solo para Apple Silicon. Trae portapapeles que salta los gestores de contraseñas, snippets, unos 40 comandos de ventanas, Script Commands de Raycast, IA con Ollama en tu Mac por defecto, [275 conversiones de archivos sin subirlos](/es/files/) y grabación de pantalla con zoom automático. La sincronización con iCloud es opcional y está desactivada por defecto. No tiene tienda de extensiones propia; las extensiones que ya instalaste con Raycast pueden ejecutarse, en parte. Más detalle en [Alternativa a Raycast](/es/alternativa-a-raycast/).'],
    ] as [string, string][],
  },
  enough: {
    h: '¿Basta Spotlight en macOS Tahoe?',
    paras: [
      'Para abrir apps, buscar archivos, recuperar algo que copiaste hace un rato y lanzar acciones de las apps de Apple, sí. Si escribes los mismos textos cada día, quieres colocar ventanas desde el teclado sin memorizar combinaciones con Fn, o necesitas integraciones con otras herramientas, un lanzador de terceros sigue aportando.',
      'Una precaución con el portapapeles: la guía de Apple avisa de que puede aparecer información sensible. Si copias contraseñas a menudo, borra el historial cuando lo necesites o usa un gestor que las excluya.',
    ],
  },
  faq: {
    h: 'Preguntas frecuentes',
    items: [
      ['¿Qué es mejor, Raycast o Alfred?', 'Depende de cómo pagues y de lo que uses. Raycast ofrece más en su plan gratis (portapapeles, snippets y ventanas) y una Store de extensiones, pero la IA y la sincronización son de suscripción. Alfred cobra un pago único por el Powerpack y destaca en automatización con Workflows.'],
      ['¿Spotlight de macOS Tahoe sustituye a Raycast?', 'Para un uso básico, a menudo sí: Spotlight ya tiene historial del portapapeles y acciones con quick keys. La guía de Apple no recoge snippets, comandos de ventanas en el buscador ni una tienda de extensiones como la de Raycast.'],
      ['¿Spotlight guarda el historial del portapapeles?', 'Sí, desde macOS Tahoe. Se abre con ⌘4 dentro de Spotlight y se puede borrar con Clear History. Apple avisa de que puede aparecer información sensible.'],
      ['¿Hay un lanzador gratis y de código abierto para Mac?', 'Sí. Ferrocast es un lanzador gratis con licencia MIT para Mac con Apple Silicon, con portapapeles, snippets, comandos de ventanas, Script Commands de Raycast y conversión de archivos en local.'],
      ['¿Qué lanzador funciona en un Mac con Intel?', 'Alfred 5 funciona desde macOS 10.14. Raycast 2 y Ferrocast necesitan Apple Silicon.'],
      ['¿Alfred tiene suscripción?', 'No. Alfred es gratis y el Powerpack es un pago único (34 £ por la licencia v5 individual).'],
    ] as [string, string][],
  },
  sources: {
    h: 'Fuentes',
    apple: [
      ['Apple: historial del portapapeles en Spotlight', 'https://support.apple.com/guide/mac-help/search-your-clipboard-history-mchl40d5b86b/mac'],
      ['Apple: acciones y quick keys en Spotlight', 'https://support.apple.com/guide/mac-help/take-actions-and-shortcuts-in-spotlight-mchl4953dfeb/mac'],
      ['Apple: atajos de mosaico de ventanas', 'https://support.apple.com/guide/mac-help/mac-window-tiling-icons-keyboard-shortcuts-mchl9674d0b0/mac'],
      ['Apple: sustituciones de texto', 'https://support.apple.com/guide/mac-help/replace-text-punctuation-documents-mac-mh35735/mac'],
      ['Apple: acciones rápidas del Finder', 'https://support.apple.com/guide/mac-help/perform-quick-actions-in-the-finder-on-mac-mchl97ff9142/mac'],
    ] as [string, string][],
    rivals: rivalSources,
  },
  more: {
    h: 'Sigue leyendo',
    links: [
      ['Alternativa gratis a Raycast', '/es/alternativa-a-raycast/'],
      ['Historial del portapapeles que ignora contraseñas', '/es/historial-portapapeles-mac/'],
      ['Migrar desde Raycast', '/es/docs/getting-started/migrating-from-raycast/'],
      ['Comprimir un vídeo a un tamaño', '/es/comprimir-video-a-un-tamano/'],
      ['Todas las conversiones', '/es/files/'],
      ['Herramientas de archivo', '/es/tools/'],
    ] as [string, string][],
  },
  disclaimer: 'Ferrocast es independiente y no está afiliado a Apple, Raycast ni Alfred.',
};
