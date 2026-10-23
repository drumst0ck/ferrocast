---
title: "Captura de pantalla"
description: "Captura un área, una ventana, la pantalla o una página con scroll como PNG. Copia, guarda, anota, añade fondo, copia el texto o fíjala. Requiere macOS 14."
sidebar:
  order: 15
---
Ferrocast hace capturas de pantalla desde el lanzador o con un atajo que elijas. Cada captura se guarda como PNG y se abre en una ventana pequeña, la ventana de captura, donde puedes copiarla, anotarla, copiar el texto que contiene o fijarla sobre las demás ventanas.

La grabación de pantalla y un editor de vídeo todavía no están disponibles. Hoy solo funcionan las capturas.

## Los cuatro comandos

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

- **macOS 14 o posterior.** En macOS 13 los comandos se ocultan. Si tienes un atajo asignado, al pulsarlo aparece una vez por sesión: `Screen capture needs macOS 14. This Mac runs macOS 13.` Si Ferrocast se compiló sin su ayudante de captura, el mensaje es `Screen capture is not available in this build.`
- **Permiso de Grabación de pantalla.** Sin él aparece este mensaje y se abre Ajustes del Sistema; no queda ningún archivo a medias:

  `Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.`

Capture Scrolling necesita además Accesibilidad, porque Ferrocast envía los eventos de scroll. Si está desactivada, el mensaje es `Scrolling capture needs Accessibility. Turn Ferrocast on in System Settings → Privacy & Security → Accessibility, then try again.`

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

:::caution[Límites]
- Solo PNG. Ajustes → Capture muestra el formato como PNG, sin otra opción. **Include cursor** dibuja el puntero en las capturas de área, ventana y pantalla.
- La captura con scroll guarda todas las tiras en memoria hasta terminar.
- Todavía no hay grabación de pantalla ni editor de vídeo. Las filas Recording de Ajustes → Capture son marcadores para una versión posterior.
:::

## Relacionado

- [Ajustes](/es/docs/launcher/settings/)
