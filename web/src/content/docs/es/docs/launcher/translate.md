---
title: "Traducir el texto seleccionado"
description: "Arrastra texto, toca Comando y suéltalo sobre un idioma: Ferrocast lo traduce con tu IA y lo sustituye en su sitio, en campos de texto y webs."
sidebar:
  order: 11
---
Selecciona texto en cualquier app, empieza a arrastrarlo, toca <kbd>⌘</kbd> y suéltalo sobre un idioma. Ferrocast envía el texto a tu proveedor de IA y deja la traducción donde estaba el texto. Si eso no es posible, muestra la traducción en una ventana y la copia.

## Traducir en tres pasos

1. Selecciona el texto y empieza a arrastrarlo, sin soltar el botón del ratón.
2. Toca la tecla de traducción, <kbd>⌘</kbd> por defecto. Se abre bajo el puntero una rueda morada con tus idiomas. No se usa <kbd>⇧</kbd> porque amplía la selección mientras arrastras.
3. Suelta sobre un idioma. Un brillo marca el texto que se está traduciendo y, un momento después, la traducción lo sustituye.

Escape cancela. <kbd>⌘</kbd> y <kbd>⌃</kbd> no hacen nada mientras esta rueda está abierta: no tiene otra rueda a la que cambiar.

## Dónde acaba la traducción

Ajustes → Translate → **After translating** decide qué pasa con el resultado.

| Opción | Qué hace |
|---|---|
| **Auto** (por defecto) | Sustituye la selección donde la app lo permite y, si no, muestra la ventana. |
| **Window** | Muestra siempre la traducción en una ventana de Ferrocast, con Copy y «Paste into app». |
| **Paste ⌘V** | Copia la traducción y pulsa <kbd>⌘</kbd> <kbd>V</kbd> en la app. Solo sirve en campos editables. |
| **Copy only** | Deja la traducción en el portapapeles. |

Con **Auto**, Ferrocast mira la app de la que salió el texto:

- **Campos de texto** (Notas, Mail, TextEdit y la mayoría de apps nativas): la selección se sustituye mediante Accesibilidad. Si la app no lo permite, Ferrocast pega con <kbd>⌘</kbd> <kbd>V</kbd>.
- **Páginas web en Safari y navegadores Chromium** (Chrome, Brave, Edge, Arc, Vivaldi, Opera): el texto seleccionado se sustituye dentro de la página. Necesita un ajuste del navegador, explicado abajo. El cambio dura hasta que recargues la página.
- **Cualquier otro caso**, o si algo falla: se abre la ventana con la traducción, que además queda en el portapapeles.

Si cambias a otra app mientras se traduce, Ferrocast nunca escribe en esa app: muestra la ventana.

## Sustituir texto en páginas web

El navegador tiene que dejar que Ferrocast ejecute un pequeño script en la página. Actívalo una vez por perfil del navegador:

- **Chrome, Brave, Edge y otros navegadores Chromium**: Ver → Opciones para desarrolladores → **Permitir JavaScript desde Eventos de Apple**. En inglés el menú es View → Developer → Allow JavaScript from Apple Events.
- **Safari**: activa Ajustes → Avanzado → Mostrar funciones para desarrolladores web y después **Permitir JavaScript desde Apple Events** en Ajustes → Desarrollador (en versiones antiguas de Safari está en el menú Desarrollo).

La primera vez macOS pregunta si Ferrocast puede controlar el navegador: permítelo. Puedes cambiarlo después en Ajustes del Sistema → Privacidad y seguridad → Automatización.

Si el ajuste está desactivado, se abre la ventana con la traducción y un aviso te dice qué opción activar.

## El efecto mientras se traduce

Mientras la IA responde, el texto que se traduce brilla con el rojo neón de Ferrocast:

- en páginas de Safari y Chromium, un contorno de neón con luces que lo recorren rodea el bloque seleccionado, y el color de selección de la propia página se oculta hasta que llega la traducción;
- en otras apps que indican dónde está la selección, se dibuja un contorno luminoso a su alrededor;
- si no, aparece una pequeña píldora «Translating…» donde soltaste.

Si la traducción falla, la página queda exactamente como estaba y tu selección vuelve. Con Reducir movimiento activado el brillo es estático. Ajustes → Translate → **Translating effect** lo desactiva.

## Motor y modelo

La traducción usa los mismos proveedores que [Ask AI](/es/docs/launcher/ask-ai/): un servidor compatible con OpenAI (con Ollama todo se queda en tu Mac), la API de Anthropic, Sign in with ChatGPT o Claude Code (tu suscripción de ChatGPT o de Claude).

En Ajustes → Translate → **Engine**:

- **Same as chat** (por defecto) usa el proveedor y el modelo de Ask AI.
- Un proveedor fijo te deja elegir su **Model**: la lista del proveedor, **Provider default** o **Custom…** para escribir un nombre. Al cambiar de proveedor el modelo se reinicia.
- **Apple** traduce en tu Mac con Apple Translation, sin proveedor de IA, clave ni servidor. El texto no se envía a ningún sitio. Requiere macOS 26 o posterior; en versiones anteriores el segmento no aparece. Mira [Traducir sin conexión con Apple](#traducir-sin-conexión-con-apple).

La dirección del servidor, las claves y los inicios de sesión se configuran en Ajustes → AI; la pestaña Translate solo elige quién traduce. El texto seleccionado se envía como dato, separado de las instrucciones, así que una palabra suelta, un título, una pregunta o una frase que parece una orden («Resume esto», «Ignora lo anterior») se traducen en lugar de contestarse o cumplirse. Después tu Mac comprueba en local el idioma de la respuesta y, si salió en otro idioma (por ejemplo cuando el texto dice «traduce esto al alemán»), Ferrocast lo pide una vez más.

## Traducir sin conexión con Apple

Apple Translation forma parte de macOS. Traduce en tu Mac, así que el texto seleccionado no sale de él y no hace falta cuenta, clave ni servidor.

1. Abre Ajustes → Translate → **Engine** y elige **Apple**.
2. La tarjeta **Apple languages** lista cada idioma de tu rueda con el estado que informa Apple: **Installed**, **Needs download** o **Not supported by Apple**.
3. Arrastra texto y suéltalo sobre un idioma como siempre. La primera traducción tras abrir la app puede tardar unos segundos mientras carga el modelo.

Que un idioma haya que descargarlo antes depende de tu versión de macOS, y lo decide Apple, no Ferrocast. En un Mac donde Apple traduce con un modelo que ya está instalado, todos los idiomas soportados salen **Installed** y no hay nada que descargar. Cuando Apple dice que un idioma **Needs download**, la tarjeta muestra una fila **Download languages**: pulsa **Open Language settings**, descárgalo en Ajustes del Sistema → General → Idioma y región → **Idiomas de traducción** y pulsa **Check again**. Si sueltas texto sobre un idioma que Apple dice que no está descargado, Ferrocast lo dice y dónde conseguirlo; si Apple no admite el idioma, sugiere otro idioma o un motor de IA.

Cada estado es un par de idiomas: el idioma con el idioma del sistema de tu Mac, o con el inglés cuando es el propio idioma del sistema. Traducir desde otro idioma puede necesitar también ese idioma.

En macOS anterior a la 26, una elección guardada de Apple vuelve al proveedor del chat, con un aviso la primera vez. Apple Translation no obedece instrucciones y el idioma de origen se detecta en tu Mac.

## Idiomas

Ajustes → Translate → **Languages** es una lista separada por comas de hasta 12 idiomas, en el orden en que aparecen en la rueda. Por defecto: inglés, español, francés, alemán, portugués, italiano, japonés y chino. Los duplicados se eliminan y una lista vacía recupera los valores por defecto. El idioma de origen lo detecta el modelo, o tu Mac con el motor de Apple.

## Privacidad

El texto seleccionado solo se envía al proveedor que elegiste, y con Ollama o con el motor de Apple no sale de tu Mac. Ferrocast no guarda ni el texto ni la traducción: la traducción solo va al portapapeles, y el propio historial de portapapeles de Ferrocast la ignora. El log registra el idioma, el proveedor, la longitud del texto y cuánto tardó, nunca el texto.

:::caution[Límites]
- Firefox no está soportado para sustituir dentro de la página.
- Las páginas del navegador como `brave://` o `chrome://` y las tiendas de extensiones no aceptan scripts, y el texto dentro de marcos (algunos editores y comentarios incrustados) no se sustituye: se abre la ventana.
- Las páginas que redibujan su texto (algunas apps web) pueden deshacer la sustitución.
- Las selecciones de más de 20.000 caracteres se rechazan.
:::

## Si no funciona

`~/Library/Application Support/Ferrocast/ferrocast.log` tiene una línea por traducción (`translate Spanish via Claude Code ok chars=… ms=…`) y otra por entrega, por ejemplo `translate deliver Spanish -> web-replace Chromium("Brave Browser")` seguida de `ok`, o de `fallback-show: js-permission` cuando el ajuste del navegador está desactivado.

Con el motor de Apple la línea dice `translate Spanish via Apple ok chars=… ms=…`. Los fallos muestran un aviso: «… is not downloaded for Apple Translation» significa que falta descargar el idioma en Ajustes del Sistema, y «Apple Translation does not support …» que Apple no tiene modelo para ese idioma.

## Relacionado

- [Ask AI](/es/docs/launcher/ask-ai/)
- [Ajustes](/es/docs/launcher/settings/)
- [La rueda de arrastre](/es/docs/files/drag-wheel/)
