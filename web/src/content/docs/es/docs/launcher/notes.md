---
title: "Notas"
description: "Una ventana flotante de notas con texto enriquecido: Markdown, imágenes, menú de barra, fijar notas, exportar, comandos de IA y dictado."
sidebar:
  order: 14
---
Notes es una ventana de texto enriquecido que flota sobre tus otras apps. Ábrela con un atajo, escribe y ciérrala. Las notas se guardan como Markdown en `notes.json`, en la carpeta de datos.

## Comandos

| Comando | Qué hace |
|---|---|
| **Notes** | Muestra u oculta la ventana de notas. Buscar «floating notes» también lo encuentra. |
| **Create Note** | Crea una nota nueva y abre la ventana. |
| **Search Notes** | Una lista de todas tus notas, con búsqueda por título y contenido. Al abrir una se muestra en la ventana flotante. |
| **Create Dictated Note** | Crea una nota nueva y empieza el dictado. |

Cada comando admite su propio atajo: selecciónalo en el lanzador y pulsa <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>,</kbd>, o elige **Set Hotkey…** en el panel de acciones (<kbd>⌘</kbd> <kbd>K</kbd>).

## Escribir una nota

Pulsa <kbd>⌘</kbd> <kbd>N</kbd> en la ventana, o usa el botón de más de la barra de herramientas, para empezar una nota nueva. La primera línea es el título y se muestra en la barra de herramientas. Solo hay una nota visible a la vez.

Una nota vacía se descarta al ocultar la ventana, salvo que sea tu única nota. Los cambios se guardan un instante después de que dejas de escribir y también al ocultar la ventana.

## Dar formato al texto

Hay cinco formas de dar formato:

1. Atajos de teclado (tablas más abajo).
2. Escribir Markdown: al teclearlo se convierte en formato.
3. El menú `/`: escribe `/` para elegir un formato de una lista.
4. **Format** en el panel de acciones (<kbd>⌘</kbd> <kbd>K</kbd>).
5. La barra de formato (Format Bar) en la parte inferior de la ventana. Se muestra por defecto. **Hide Format Bar** en el panel de acciones la oculta y **Show Format Bar** la vuelve a mostrar.

Un pie en la parte inferior de la ventana muestra los atajos principales: New <kbd>⌘</kbd> <kbd>N</kbd>, Browse <kbd>⌘</kbd> <kbd>P</kbd>, Pin <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>P</kbd> y Actions <kbd>⌘</kbd> <kbd>K</kbd>. Puedes hacer clic en sus elementos y, en una ventana estrecha, los menos importantes se omiten.

La ventana de notas usa los mismos colores que el lanzador y sigue el ajuste Appearance de Settings (System, Dark o Light).

### Formatos de párrafo

| Formato | Atajo | Markdown |
|---|---|---|
| Heading 1 | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>1</kbd> | `#` y un espacio |
| Heading 2 | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>2</kbd> | `##` y un espacio |
| Heading 3 | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>3</kbd> | `###` y un espacio |
| Code block | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>C</kbd> | tres acentos graves y un espacio, o `~~~` |
| Blockquote | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>B</kbd> | `>` y un espacio |
| Ordered list | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>7</kbd> | `1.` y un espacio |
| Bullet list | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>8</kbd> | `*` o `-` y un espacio |
| Task list | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>9</kbd> | `[]` o `[x]` y un espacio |
| Horizontal rule | ninguno | `---` o `___` y un espacio |

Para marcar o desmarcar una tarea, pon el cursor en su línea y pulsa <kbd>⌘</kbd> <kbd>↵</kbd>. Las listas se pueden sangrar.

### Formatos de texto

| Formato | Atajo | Markdown |
|---|---|---|
| Bold | <kbd>⌘</kbd> <kbd>B</kbd> | `**texto**` o `__texto__` |
| Italic | <kbd>⌘</kbd> <kbd>I</kbd> | `*texto*` o `_texto_` |
| Strikethrough | <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>S</kbd> | `~~texto~~` |
| Underline | <kbd>⌘</kbd> <kbd>U</kbd> | ninguno (se guarda como `<u>texto</u>`) |
| Code | <kbd>⌘</kbd> <kbd>E</kbd> | `` `texto` `` |
| Link | <kbd>⌘</kbd> <kbd>L</kbd> | `[texto](url)` |

Los enlaces no están en el menú `/`. Los enlaces se abren con <kbd>⌘</kbd> y clic.

### El menú `/` y los emoji

Escribe `/` para abrir un menú de formatos y fíltralo al teclear. Escribe `:` para abrir una lista de emoji y fíltrala igual.

## Imágenes

Puedes poner una imagen en una nota de tres maneras:

- Pégala con <kbd>⌘</kbd> <kbd>V</kbd>: una captura de pantalla, una imagen copiada de un navegador o archivos de imagen copiados en Finder.
- Arrastra archivos de imagen sobre la nota. Quedan después de la línea donde los sueltas.
- Elige **Insert Image…** (<kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>I</kbd>, o el menú del clic derecho) y escoge archivos.

Valen PNG, JPEG, GIF, HEIC y WebP, hasta 20 MB cada una. Un archivo mayor o que no sea una imagen se rechaza con un mensaje que lo nombra; las demás imágenes que añadiste se insertan igualmente. Si pegas texto que lleva una imagen al lado (de una web o de un documento), el texto se pega como siempre.

Ferrocast copia cada imagen a la carpeta `attachments` de la carpeta de datos con un nombre hecho a partir de su contenido, así que la misma imagen se guarda una sola vez. La nota recibe un enlace Markdown estándar en su propia línea, como `![nombre](attachments/3f2a….png)`, y la imagen se dibuja en la nota con su tamaño, hasta 480 puntos de ancho y manteniendo la proporción. Las notas siguen siendo Markdown plano: el enlace es una ruta relativa y solo se muestran archivos de esa carpeta.

Para quitar una imagen, selecciónala y pulsa Borrar. Un archivo que ninguna nota enlaza se elimina a los 7 días, de modo que deshacer y Eliminadas recientemente siguen encontrándolo. Si falta un archivo (por ejemplo, aún no ha llegado a este Mac) se muestra un marcador y la imagen aparece cuando el archivo está aquí.

La ventana de Notes edita solo las notas de Ferrocast. Las notas de tu bóveda de Obsidian no se editan aquí, así que Ferrocast nunca copia una imagen a tu bóveda ni mueve imágenes de ella a su carpeta.

## Organizar las notas

- **Browse Notes**: <kbd>⌘</kbd> <kbd>P</kbd> o el botón de la barra de herramientas lista todas las notas.
- **Fijar**: <kbd>⇧</kbd> <kbd>⌘</kbd> <kbd>P</kbd> fija o suelta la nota abierta (<kbd>⌘</kbd> <kbd>.</kbd> en la lista de Search Notes). Las notas fijadas quedan arriba en Browse Notes y en Search Notes.
- **Notas fijadas**: con la ventana abierta, de <kbd>⌘</kbd> <kbd>1</kbd> a <kbd>⌘</kbd> <kbd>9</kbd> abren la primera a la novena nota fijada.
- **Historial**: <kbd>⌘</kbd> <kbd>[</kbd> y <kbd>⌘</kbd> <kbd>]</kbd> van atrás y adelante entre las notas que abriste, como en un navegador.

## Eliminar notas

En el panel de acciones, **Delete Note** y **Delete All Notes** mueven las notas a **Recently Deleted**. Abre Recently Deleted desde el mismo panel y elige una nota para restaurarla. Las notas eliminadas se conservan 30 días y después se borran definitivamente.

## Zoom

<kbd>⌘</kbd> <kbd>+</kbd> acerca, <kbd>⌘</kbd> <kbd>-</kbd> aleja y <kbd>⌘</kbd> <kbd>0</kbd> vuelve al tamaño real. El zoom también está en Ajustes → Notes.

## El panel de acciones

<kbd>⌘</kbd> <kbd>K</kbd> abre el panel de acciones de la nota abierta. Escribe para buscar, también dentro de sus submenús (por ejemplo Format › Bold o Export › Markdown). Contiene:

- New Note, Browse Notes, Pin o Unpin Note, Navigate Back y Navigate Forward.
- Format, Show o Hide Format Bar y Start o Stop Dictation.
- Copy As, Export, Create Quicklink y AI Commands.
- Zoom In, Zoom Out y Actual Size.
- Delete Note, Delete All Notes y Recently Deleted.

Las acciones que no se pueden aplicar no aparecen en la lista.

## Exportar y copiar

**Export** escribe la nota como **Plain Text**, **Markdown** o **HTML**. El archivo va a tu carpeta Descargas y se muestra en Finder; no hay diálogo de guardado. **Copy As** deja la nota en el portapapeles en los mismos tres formatos. Ninguno incluye los archivos de imagen de una nota: Markdown conserva el enlace y copiar como texto plano deja fuera las imágenes.

## Quicklink y URL

**Create Quicklink** añade un [quicklink](/es/docs/launcher/quicklinks/) que abre esa nota desde cualquier sitio. Usa estas URL, que también puedes usar desde otras apps:

| URL | Qué hace |
|---|---|
| `ferrocast://notes/<id>` | Abre esa nota. Una nota desconocida o eliminada solo muestra un mensaje. |
| `ferrocast://notes/new` | Crea una nota y la abre. |
| `ferrocast://notes` | Muestra u oculta la ventana de notas. |

## Comandos de IA

**AI Commands** en el panel de acciones ejecuta uno de estos sobre el texto seleccionado, o sobre toda la nota si no hay selección:

- Fix Spelling and Grammar, Improve Writing, Make Shorter, Make Longer.
- Change Tone: Professional, Casual, Friendly, Confident o Straightforward.
- Explain This y Translate to English.

**Summarize** y **Continue Writing** trabajan siempre sobre toda la nota.

Usan el proveedor que configuraste en Ajustes → AI (consulta [Ask AI](/es/docs/launcher/ask-ai/)) y le envían el texto. Summarize y Continue Writing añaden su resultado al final de la nota, y Explain This lo añade debajo de la selección. Nunca sustituyen tu texto. Los demás comandos sustituyen el texto sobre el que trabajaron. Cada resultado se aplica como un solo paso de deshacer (<kbd>⌘</kbd> <kbd>Z</kbd>) y se conserva el formato Markdown. Solo se ejecuta un comando a la vez. Si editas la nota mientras se ejecuta, el resultado se descarta.

## Dictado

Elige **Start Dictation** en el panel de acciones, o ejecuta **Create Dictated Note**, y habla. La primera vez, macOS pide acceso al micrófono y al reconocimiento de voz. Ferrocast usa el reconocimiento en el dispositivo cuando tu sistema lo admite. El dictado se detiene al ocultar la ventana, cambiar de nota, eliminar la nota, editar el texto dictado o elegir **Stop Dictation**.

## Ajustes

Ajustes → Notes tiene tres opciones:

| Opción | Qué hace |
|---|---|
| **Show Notes Toggle Button in Menu Bar** | Añade a la barra de menús un botón que muestra u oculta la ventana de notas. Desactivado por defecto. |
| **Escape Key Loses Focus** | Esc quita el foco de la ventana de notas en lugar de cerrarla. Desactivado por defecto. |
| **Zoom** | Tamaño del texto, del 50 % al 300 %. Por defecto 100 %. |

:::caution[Límites]
- No hay límite en el número de notas.
- <kbd>esc</kbd> cierra la ventana salvo que actives **Escape Key Loses Focus**.
- <kbd>⌘</kbd> <kbd>0</kbd> significa tamaño real, así que las notas fijadas usan de <kbd>⌘</kbd> <kbd>1</kbd> a <kbd>⌘</kbd> <kbd>9</kbd>.
- Los comandos de IA y el dictado dependen de tu proveedor y de los permisos de macOS.
- Las imágenes se dibujan en su propia línea o dentro de un párrafo, nunca con el texto rodeándolas, y no se pueden redimensionar ni recortar en la nota.
:::

## Relacionado

- [Atajos de teclado](/es/docs/launcher/keyboard-shortcuts/)
- [Ajustes](/es/docs/launcher/settings/)
- [Quicklinks](/es/docs/launcher/quicklinks/)
- [Datos y privacidad](/es/docs/reference/data-and-privacy/)
