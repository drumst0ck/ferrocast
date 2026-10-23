---
title: "Captura de pantalla"
description: "Haz capturas y graba la pantalla desde el lanzador: área, ventana, pantalla completa o página con scroll. Requiere macOS 14 y Grabación de pantalla."
sidebar:
  order: 15
---
Ferrocast hace capturas de pantalla desde el lanzador o con un atajo que elijas. Cada captura se guarda como PNG y se abre en una ventana pequeña, la ventana de captura, donde puedes copiarla, anotarla, copiar el texto que contiene o fijarla sobre las demás ventanas.

También graba la pantalla. Una grabación se guarda como una carpeta de proyecto con el vídeo y un registro de lo que hiciste con el ratón. La edición y la exportación llegarán más adelante: hoy puedes grabar, explorar tus grabaciones y mostrarlas en el Finder.

## Los cuatro comandos de captura

Escribe cualquiera en el lanzador. Todos llevan el subtítulo «Capture».

| Comando | Qué hace |
|---|---|
| **Capture Area** | Oscurece cada pantalla y muestra una cruz y el tamaño en píxeles (`W × H`). Arrastra para elegir el área. <kbd>Espacio</kbd> cambia al modo ventana y <kbd>Esc</kbd> cancela sin guardar nada. |
| **Capture Window** | La misma superposición, ya en modo ventana: haz clic en la ventana que quieres. |
| **Capture Screen** | Captura la pantalla completa. |
| **Capture Scrolling** | Elige un área y Ferrocast la desplaza y une las partes en una sola imagen alta. Mira más abajo. |

Ninguno tiene atajo por defecto. Asigna uno en Ajustes → Capture → Shortcuts.

Las capturas se guardan con el tamaño nativo en píxeles de la pantalla (el doble de los puntos en una pantalla Retina) como `Capture AAAA-MM-DD at HH.MM.SS.png`. Si ese nombre ya existe, se añade ` 2`. La carpeta por defecto es `~/Pictures/Ferrocast Captures`.

## Requisitos

Sirven para las capturas y para las grabaciones.

- **macOS 14 o posterior.** En macOS 13 los comandos se ocultan. Si tienes un atajo asignado, al pulsarlo aparece una vez por sesión: `Screen capture needs macOS 14. This Mac runs macOS 13.` Si Ferrocast se compiló sin su ayudante de captura, el mensaje es `Screen capture is not available in this build.`
- **Permiso de Grabación de pantalla.** Sin él aparece este mensaje y se abre Ajustes del Sistema; no queda ningún archivo a medias:

  `Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.`

Capture Scrolling necesita además Accesibilidad, porque Ferrocast envía los eventos de scroll. Si está desactivada, el mensaje es `Scrolling capture needs Accessibility. Turn Ferrocast on in System Settings → Privacy & Security → Accessibility, then try again.`

Grabar también necesita Grabación de pantalla. Sin ella aparece el mismo mensaje, se abre Ajustes del Sistema y se elimina la carpeta de proyecto a medias.

## La ventana de captura

Tras una captura, Ajustes → Capture → **After capture** decide qué pasa: **Show** abre la ventana de captura, **Copy** solo copia la imagen y **Save** solo guarda el archivo. La ventana tiene seis botones. Todos trabajan sobre el archivo guardado.

| Botón | Atajo |
|---|---|
| **Copy** (copia la imagen y cierra) | <kbd>↵</kbd> |
| **Save** | <kbd>⌘</kbd> <kbd>S</kbd> |
| **Annotate…** (abre el editor de anotaciones) | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>A</kbd> |
| **Background…** (abre el editor de fondo) | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>B</kbd> |
| **Copy Text** | <kbd>⌘</kbd> <kbd>T</kbd> |
| **Pin** | <kbd>⌘</kbd> <kbd>P</kbd> |

## Copy Text

Copy Text lee el texto de la imagen con el framework Vision de macOS, en tu Mac, y lo deja en el portapapeles en orden de lectura. Un aviso dice `Copied N characters` o `No text found`. La imagen no se envía a ningún sitio.

## Pin

Pin abre la imagen en una ventana sin bordes que se queda sobre las demás, con su tamaño de captura en puntos. Arrástrala por el fondo. Haz zoom del 25 % al 400 % con la rueda, el gesto de pellizco, <kbd>+</kbd> y <kbd>-</kbd>; <kbd>0</kbd> vuelve al tamaño capturado. <kbd>Esc</kbd> la cierra. Puedes tener varias a la vez.

## Captura con scroll

Elige el área que se desplaza y Ferrocast la desplaza por ti, toma una tira tras cada paso y une las tiras en una sola imagen.

- Se detiene cuando el contenido deja de moverse (dos tiras idénticas) o tras 60 pantallas, con el aviso `Scrolling capture stopped at 60 screens`.
- Si pierde la referencia del solape, guarda las pantallas capturadas hasta ese punto y lo avisa.
- <kbd>Esc</kbd> la cancela. Hacer clic no.
- El puntero del ratón nunca se dibuja en las tiras.

## Grabación de pantalla

### Los comandos

| Comando | Qué hace |
|---|---|
| **Record Screen** | Graba una pantalla completa. |
| **Record Area** | Elige un área, como en Capture Area, y la graba. |
| **Record Window** | Elige una ventana, como en Capture Window, y la graba. |
| **Open Last Recording** | Muestra tu grabación más reciente en el Finder. Mira la nota sobre el editor más abajo. |
| **Recordings** | Lista tus grabaciones, la más reciente primero. |

Todos llevan el subtítulo «Capture» y ningún atajo por defecto. Asigna atajos en Ajustes → Capture → Shortcuts.

### Empezar y parar

Tras elegir el objetivo, corre una cuenta atrás (3 s por defecto; Ajustes → Capture → **Recording countdown** ofrece Off, 3 s y 5 s). Ejecutar el comando otra vez durante la cuenta atrás la cancela. Después aparece un panel pequeño con el tiempo transcurrido, **Pause** (pasa a **Resume**) y **Stop**. El panel no sale en el vídeo.

Mientras grabas, el icono de la barra de menús muestra `●  mm:ss` y el menú empieza con **Stop Recording**. Para parar usa el panel o ese elemento del menú. Mientras una grabación está en marcha no puedes empezar otra. Un aviso te dice que el archivo está guardado, con su nombre y duración.

La frecuencia por defecto es 60 fps; Ajustes → Capture → **Recording frame rate** ofrece también 30 fps. La grabación dura lo mismo que el tiempo real entre el final de la cuenta atrás y Stop, aunque la pantalla no cambie.

### Qué se guarda

Cada grabación es una carpeta llamada `Recording AAAA-MM-DD at HH.MM.SS.ferrocast` dentro de `~/Movies/Ferrocast Recordings`. Para usar otra carpeta, define `capture.recordings_folder` en `settings.json`; todavía no hay fila en Ajustes. Dentro hay:

| Archivo | Qué contiene |
|---|---|
| `screen.mov` | El vídeo. El puntero del ratón no se dibuja en él: el editor lo dibujará después a partir de `events.json`. |
| `events.json` | Un registro de la posición del cursor (unas 60 veces por segundo), los clics, el scroll y la posición de las ventanas. De las teclas solo guarda los atajos (una tecla pulsada con un modificador distinto de <kbd>⇧</kbd>) para poder mostrarlos. Lo que escribes normalmente nunca se guarda, salvo el instante de cada pulsación, así que una grabación no puede contener una contraseña. |
| `project.json` | Los ajustes de la grabación y su duración. |
| `thumb.png` | Una imagen fija de la grabación, usada en la lista Recordings. |

Todo se queda en tu Mac.

### Monitorización de entrada

Monitorización de entrada (Input Monitoring) solo hace falta para registrar los atajos de teclado. Sin ella la grabación funciona igual y al empezar ves este mensaje: `Keyboard shortcuts will not be shown: Input Monitoring is off.` Los clics, el scroll y el cursor no la necesitan.

### Poco espacio en disco

Con menos de 2 GB libres aparece el aviso `Low disk space: N GB left`. Con menos de 200 MB la grabación se detiene sola, de forma limpia, y conserva el archivo. El aviso dice `Recording stopped: low disk space.` seguido del nombre y la duración del proyecto.

### Recordings y Open Last Recording

**Recordings** lista los proyectos de la carpeta de grabaciones, el más reciente primero, con su duración y tamaño.

| Tecla | Acción |
|---|---|
| <kbd>↵</kbd> | Mostrar en el Finder |
| <kbd>⌘</kbd> <kbd>⌫</kbd> | Mover a la Papelera (pide confirmación) |
| <kbd>Esc</kbd> | Volver |

El editor todavía no está disponible. Por eso **Open Last Recording** solo muestra el proyecto más reciente en el Finder y dice `Showing NOMBRE in Finder. The editor is not available yet.` Si no hay ninguna grabación, dice `No recordings yet`. La edición y la exportación llegarán más adelante; no hay fecha.

:::caution[Límites]
- Solo PNG. Ajustes → Capture muestra el formato como PNG, sin otra opción. **Include cursor** dibuja el puntero en las capturas de área, ventana y pantalla.
- La captura con scroll guarda todas las tiras en memoria hasta terminar.
- Las grabaciones todavía no se pueden editar ni exportar, y no hay grabación de cámara ni de audio. Ábrelas desde el Finder.
- Una grabación se pierde si sales de Ferrocast mientras graba. Párala antes.
:::

## Relacionado

- [Ajustes](/es/docs/launcher/settings/)
