---
title: "Captura de pantalla"
description: "Haz capturas, graba la pantalla, edítala con zoom automático y exporta a MP4 o GIF desde el lanzador. Requiere macOS 14 y Grabación de pantalla."
sidebar:
  order: 15
---
Ferrocast hace capturas de pantalla desde el lanzador o con un atajo que elijas. Cada captura se guarda como PNG y se abre en una ventana pequeña, la ventana de captura, donde puedes copiarla, anotarla, copiar el texto que contiene o fijarla sobre las demás ventanas.

También graba la pantalla. Una grabación se guarda como una carpeta de proyecto con el vídeo y un registro de lo que hiciste con el ratón. Después puedes editarla en Ferrocast, con zooms automáticos en tus clics, y exportarla como MP4 o como GIF.

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
| **Open Last Recording** | Abre tu grabación más reciente en el editor. |
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
| `screen.mov` | El vídeo. El puntero del ratón no se dibuja en él: el editor lo dibuja a partir de `events.json`. |
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
| <kbd>↵</kbd> | Abrir en el editor |
| <kbd>⌘</kbd> <kbd>↵</kbd> | Mostrar en el Finder |
| <kbd>⌘</kbd> <kbd>⌫</kbd> | Mover a la Papelera (pide confirmación) |
| <kbd>Esc</kbd> | Volver |

**Open Last Recording** abre la grabación más reciente en el editor. Si no hay ninguna, dice `No recordings yet`.

## Editar una grabación

El editor muestra la vista previa a la izquierda y un panel con cuatro pestañas a la derecha. Debajo de la vista previa hay una línea de tiempo (una regla, una fila para recortes y cambios de velocidad y otra para los zooms) con el cabezal, y una fila con **Play**/**Pause**, **Undo**, **Redo**, **Export…** y el tiempo. El tiempo se lee `tiempo de salida / duración de salida · tiempo original`, así ves la duración tras tus cambios. En el extremo derecho de esa fila aparecen el número de zooms y el estado de guardado. <kbd>Esc</kbd> cierra el editor y escribe lo que aún no se haya guardado.

El cursor que ves lo dibuja el editor a partir de los eventos grabados en `events.json`. No está en `screen.mov`, y por eso puedes cambiarle el tamaño, suavizarlo o quitarlo.

### Reproducción y teclas

| Tecla | Acción |
|---|---|
| <kbd>Espacio</kbd> | Reproducir o pausar |
| <kbd>←</kbd> / <kbd>→</kbd> | Un fotograma atrás o adelante |
| <kbd>⇧</kbd> <kbd>←</kbd> / <kbd>⇧</kbd> <kbd>→</kbd> | Un segundo atrás o adelante |
| <kbd>Inicio</kbd> / <kbd>Fin</kbd> | Ir al principio o al final |
| <kbd>⌘</kbd> <kbd>Z</kbd> / <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>Z</kbd> | Deshacer / Rehacer |
| <kbd>⌘</kbd> <kbd>E</kbd> | Export… |

Haz clic o arrastra sobre la línea de tiempo para mover el cabezal. Mientras escribes en un campo numérico del panel, solo <kbd>⌘</kbd> <kbd>E</kbd> sigue funcionando. El paso de un fotograma usa la frecuencia de Ajustes → Capture → **Recording frame rate**.

### Zoom automático

El zoom automático hace zoom hacia tus clics. Los clics cercanos en el tiempo y en el espacio forman un grupo. Un zoom empieza 0,4 s antes del primer clic del grupo y vuelve a la pantalla completa tras 1,2 s sin clics ni movimiento del cursor. El nivel por defecto es 2×.

Al abrir una grabación que no tiene zooms, Ferrocast los calcula y los aplica solo en memoria. No se escribe nada en `project.json` hasta que cambias algo. Si exportas una grabación que no has tocado, antes se guarda su `project.json` con esos zooms.

El zoom automático viene activado. `capture.auto_zoom` y `capture.zoom_scale` en `settings.json` lo desactivan o fijan el nivel (de 1× a 6×); todavía no hay fila en Ajustes. El botón **Auto zoom** de la pestaña Zoom usa el mismo nivel.

### El panel

Todos los cambios se pueden deshacer y rehacer (<kbd>⌘</kbd> <kbd>Z</kbd>, <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>Z</kbd> o los botones **Undo** y **Redo**). Los cambios del mismo tipo hechos en menos de un segundo, como arrastrar un control deslizante, cuentan como un paso. Un cambio que dejaría el proyecto inválido se rechaza y el motivo aparece en rojo sobre los controles.

**Zoom**

- **Add at playhead** añade un zoom manual de 2 s, más corto si el siguiente zoom está cerca. Si el cabezal está dentro de un zoom aparece `There is already a zoom at the playhead.`
- **Auto zoom** sustituye los zooms automáticos por otros nuevos. Los manuales se quedan y ganan en cualquier solape.
- Haz clic en un zoom de la lista para editarlo: **Start**, **End**, **Scale** (de 1× a 6×), **Focus** (X e Y, en %), **Ease in** y **Ease out** (de 0 a 3 s). Editar un zoom lo convierte en manual, así que Auto zoom lo respeta.
- **Pick on preview** fija el foco donde haces clic en la vista previa. **Follow cursor** hace que el zoom vuelva a seguir el puntero. **Delete zoom** lo elimina.

**Cursor**

- **Default**, **None** y **Hidden**: el puntero, o ningún puntero (None y Hidden hacen lo mismo).
- **Size** (de 0,5× a 4×), **Smoothing** (de 0 a 100 %), **Click ring** (un anillo donde hiciste clic) y **Motion blur** (de 0 a 200 %).

**Style**

- **Aspect**: 16:9, 4:3, 1:1, 9:16 o Source.
- **Background**: **Colour**, **Gradient** o **Blur** (la propia grabación, desenfocada), con selectores de color y un **Angle** para los degradados.
- Ocho degradados predefinidos: Aurora, Dusk, Ocean, Graphite, Sand, Lilac, Forest y Rose. Un preajuste se guarda como degradado, no como imagen de fondo.
- **Padding** (de 0 a 45 %), **Corners** (de 0 a 64 px) y la sombra: **Shadow** (desenfoque, de 0 a 120 px), **Opacity** y **Offset** (de 0 a 80 px).

**Clips**

- **Trim in** corta todo lo anterior al cabezal y **Trim out** todo lo posterior. **Split** divide el segmento en el cabezal. **Delete segment** elimina el segmento del cabezal.
- **Speed**: elige 0.25×, 0.5×, 1×, 2×, 4×, 8× o 16× (o escribe un valor) y pulsa **Apply to segment**.
- Debajo, la lista de segmentos (haz clic en uno para mover el cabezal) y la duración antes y después de tus cambios.

### Guardado

El editor escribe `project.json` solo, 1 s después de tu último cambio, como un archivo temporal que se renombra al terminar. El estado de guardado dice `Saved` o `Saving…`; si falla una escritura muestra el texto del error y vuelve a intentarlo. Una grabación que solo has abierto no se reescribe.

### Export…

**Export…** (o <kbd>⌘</kbd> <kbd>E</kbd>) abre una hoja. Primero guarda `project.json`, porque la exportación renderiza el proyecto guardado.

| Formato | Opciones |
|---|---|
| **MP4** | Códec **H.264** o **HEVC**. Resolución **Source**, **1080p** o **4K** (el lado largo mide 1920 o 3840 px), nunca por encima de 3840×2160. Frecuencia **30 fps** o **60 fps**. Calidad **Standard** o **High**. |
| **GIF** | Ancho **480 px**, **720 px** o **960 px**. Frecuencia **10 fps** o **15 fps**. |

La hoja muestra el resultado, por ejemplo `1920×1080 · 2:31 · 4530 frames`, y el destino. **Choose…** abre un diálogo para guardar. El nombre por defecto es el del proyecto con `.mp4` o `.gif`, junto a la carpeta del proyecto; si ya existe, se añade un número. Mientras se exporta, una barra muestra el porcentaje y el tiempo restante. **Cancel** (o <kbd>Esc</kbd>) la detiene y no deja ningún archivo a medias. Al terminar, un aviso dice `Exported NOMBRE` y el archivo se muestra en el Finder. Cerrar el editor cancela una exportación en curso. La duración exportada es la de la línea de tiempo editada, con los recortes y cambios de velocidad incluidos.

Un GIF guarda todos sus fotogramas en memoria, así que su duración tiene un límite. Con 960 px de ancho (16:9, 15 fps) el límite es de 20 segundos; un GIF más estrecho o con menos fps puede durar más, por ejemplo unos 80 segundos a 480 px. Si te pasas, la hoja dice:

`This edit is too long for a GIF: the limit is N seconds. Trim it or export an MP4.`

(N es el límite para el ancho, la frecuencia y el aspecto que elegiste. El texto del error está solo en inglés.)

:::caution[Límites]
- Solo PNG. Ajustes → Capture muestra el formato como PNG, sin otra opción. **Include cursor** dibuja el puntero en las capturas de área, ventana y pantalla.
- La captura con scroll guarda todas las tiras en memoria hasta terminar.
- Las exportaciones todavía no llevan audio. La grabación de cámara y de micrófono y la edición de la transcripción no están disponibles.
- El editor no usa imágenes de fondo: los preajustes de Style son degradados.
- Una grabación se pierde si sales de Ferrocast mientras graba. Párala antes.
:::

## Relacionado

- [Ajustes](/es/docs/launcher/settings/)
