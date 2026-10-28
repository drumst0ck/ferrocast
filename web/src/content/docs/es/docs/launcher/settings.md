---
title: "Ajustes y copia de seguridad"
description: "Ajustes de Ferrocast tiene diez pestañas, de General a Advanced. Elige el idioma, define atajos e inicio al abrir sesión y haz copias de seguridad."
sidebar:
  order: 12
---
Ajustes es una barra lateral con diez pestañas: General, Clipboard, Snippets, AI, Translate, Capture, Notes, Scripts, Integrations y Advanced. <kbd>⌘</kbd> <kbd>1</kbd> a <kbd>⌘</kbd> <kbd>9</kbd> y <kbd>⌘</kbd> <kbd>0</kbd> saltan a ellas en ese orden. Cada ajuste es una fila con un título, una descripción en lenguaje sencillo y su control (interruptores animados, selectores segmentados, teclas).

Ábrelos con <kbd>⌘</kbd> <kbd>,</kbd>. También hay un icono en la barra de menús.

## Idioma

Ferrocast habla inglés, español, alemán, francés, portugués de Brasil, japonés y chino simplificado. Por defecto sigue el orden de idiomas de macOS (Ajustes del Sistema → General → Idioma y región); el portugués de Portugal usa el texto brasileño y un idioma sin traducción, como el chino tradicional, vuelve al inglés.

Para elegirlo tú, abre Ajustes → General → **Idioma** y selecciona Sistema o uno de los siete idiomas. Se aplica al instante, sin reiniciar, al lanzador, los Ajustes, el tour, el menú de la barra de menús y las notificaciones. La elección se guarda solo en este Mac: no se sincroniza con iCloud. Un texto sin traducir aparece en inglés. Los nombres que escribes (fragmentos, enlaces rápidos, notas, scripts) y los títulos de las extensiones de Raycast nunca se traducen.

## Qué puedes configurar

- El atajo del lanzador (por defecto <kbd>⌥</kbd> <kbd>Space</kbd>) y atajos por comando.
- Apariencia (Ajustes → General): System, Dark o Light, o tus propios colores con vista previa, avisos de contraste y exportación o importación como archivo (consulta [Temas personalizados](/es/docs/launcher/themes/)).
- Abrir al iniciar sesión mediante un LaunchAgent; se aplica en el siguiente inicio de sesión.
- Las teclas de la rueda de arrastre y su activación en Ajustes → General (consulta [Rueda de arrastre](/es/docs/files/drag-wheel/)).
- Actualizaciones (Ajustes → General): comprobar automáticamente, preguntar antes o instalar automáticamente, y Check now (consulta [Instalar](/es/docs/getting-started/install/#updates)).
- Todas las opciones de traducción están en Ajustes → Translate: la tecla de traducción (Comando por defecto), los idiomas, qué pasa tras traducir y el proveedor y modelo que traducen (consulta [Traducir el texto seleccionado](/es/docs/launcher/translate/)).
- Ajustes → Capture tiene la carpeta de capturas (vacía usa `~/Pictures/Ferrocast Captures`), el formato (solo PNG), **After capture** (Show, Copy o Save), **Include cursor** la **Recording frame rate** (30 o 60 fps), la **Recording countdown** (Off, 3 s o 5 s) y una fila de atajo para Capture Area, Capture Window, Capture Screen, Capture Scrolling, Record Screen, Record Area y Record Window, todas vacías por defecto (consulta [Captura de pantalla](/es/docs/launcher/screen-capture/)).
- Ajustes → Notes tiene tres opciones para la ventana flotante de notas: un botón en la barra de menús que la muestra u oculta, si Esc quita el foco en lugar de cerrarla, y el zoom (consulta [Notas](/es/docs/launcher/notes/)).
- El proveedor de IA, el modelo y las claves (consulta [Ask AI](/es/docs/launcher/ask-ai/)).
- Copia de seguridad: exporta tus datos (el historial del portapapeles no se incluye) e importa snippets desde una exportación JSON de Raycast.

:::caution[Límites]
Abrir al iniciar sesión se aplica en el siguiente inicio de sesión. Los guardados fallidos se notifican.
:::

## Relacionado

- [Notas](/es/docs/launcher/notes/)
- [Atajos de teclado](/es/docs/launcher/keyboard-shortcuts/)
- [Datos y privacidad](/es/docs/reference/data-and-privacy/)
