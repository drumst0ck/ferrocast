---
title: "Obsidian: buscar y crear notas"
description: "Busca en tu vault de Obsidian desde el lanzador con vista previa, y crea notas o añade a la nota diaria sin salir de Ferrocast. Todo se lee en local."
sidebar:
  order: 15
---
Search Obsidian encuentra las notas de tu vault desde el lanzador, muestra una vista previa junto a la lista y abre la nota en Obsidian. Ferrocast lee directamente los archivos Markdown de tu Mac, así que también funciona con Obsidian cerrado. Solo las acciones que abren Obsidian necesitan la aplicación.

## Elige tu vault

1. Abre Ajustes → Integrations y busca el grupo **Obsidian**.
2. Elige un vault en **Vault**. Ferrocast muestra por nombre los vaults que Obsidian conoce. Si el tuyo no aparece, usa **Choose Folder…** en **Vault folder** y selecciona la carpeta del vault.
3. La fila **Obsidian app** indica si Obsidian se encontró en este Mac.

Mientras no haya un vault elegido, Search Obsidian muestra **Choose your Obsidian vault** y una sola fila, **Open Settings → Integrations**, que te lleva a ese grupo.

## Buscar

Abre **Search Obsidian** desde el lanzador. Puedes asignarle un alias o un atajo de teclado como a cualquier otro comando.

- Con el cuadro de búsqueda vacío, la lista muestra las 30 notas modificadas más recientemente, bajo **Recent**.
- Al escribir se compara con el título de la nota, sus `aliases` del frontmatter y su ruta. El título de una nota es el `title` del frontmatter; si no existe, su primer encabezado `# `; si tampoco, el nombre del archivo. Las coincidencias aparecen bajo **Notes**, de mejor a peor, hasta 30.
- Con 3 o más caracteres, Ferrocast también busca dentro de las notas. Espera un cuarto de segundo tras tu última tecla y lista bajo **In notes** las notas que contienen tu texto y no coincidieron por título.

Cada fila muestra el título, la carpeta donde está (vacía para las notas en la raíz del vault) y cuándo se modificó por última vez: «2h ago», «yesterday» o la fecha.

La primera vez que abres el comando en una sesión, Ferrocast indexa el vault en segundo plano y la lista muestra **Indexing your vault…** hasta que está listo. Las siguientes veces, el último índice aparece al instante y se actualiza en segundo plano, de modo que las notas añadidas desde entonces aparecen un momento después.

## Vista previa

La nota seleccionada aparece en un panel a la derecha: su título y su ruta, y después su contenido con encabezados, viñetas, listas numeradas, casillas de tareas, citas, bloques de código y separadores. El frontmatter se oculta y los `[[wikilinks]]` se muestran como texto normal. La vista previa lee los primeros 200 KB de la nota. Pulsa <kbd>⌘</kbd> <kbd>Y</kbd> para ocultar o mostrar el panel; con el panel oculto la lista ocupa todo el ancho y no se lee nada.

Si una nota se movió o se borró después de crear el índice, o no se puede leer, el panel muestra **Could not read this note.**

## Acciones

Pulsa <kbd>↵</kbd> para abrir la nota seleccionada en Obsidian, o <kbd>⌘</kbd> <kbd>K</kbd> para ver todas las acciones.

| Acción | Atajo | Qué hace |
|---|---|---|
| **Open in Obsidian** | <kbd>↵</kbd> | Abre la nota en tu vault mediante un enlace `obsidian://open`. |
| **Reveal in Finder** | <kbd>⌘</kbd> <kbd>↵</kbd> | Muestra el archivo en el Finder. |
| **Copy Wikilink** | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> | Copia `[[Nombre de la nota]]`. Obsidian resuelve los enlaces por nombre de archivo, así que cuando el título difiere del nombre del archivo se copia `[[Nombre del archivo\|Título]]` y el enlace sigue funcionando. |
| **Copy Path** | <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> | Copia la ruta completa del archivo. |
| **Quick Look** | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>Y</kbd> | Muestra el archivo en el panel Quick Look del sistema. <kbd>⌘</kbd> <kbd>Y</kbd> por sí solo oculta o muestra el panel de vista previa. |
| **Search in Obsidian** | | Ejecuta el texto del cuadro de búsqueda en la búsqueda propia de Obsidian (`obsidian://search`). |

Los enlaces de más de 8.000 caracteres se rechazan con un aviso en lugar de recortarse. Si Ferrocast ha comprobado que Obsidian no está instalado, **Open in Obsidian** y **Search in Obsidian** muestran «Obsidian is not installed. Download it from obsidian.md.» y no abren nada. Mientras esa comprobación no haya terminado, Ferrocast intenta abrir el enlace de todos modos y deja que responda macOS.

## Crear notas

Tres comandos más envían texto a tu vault. Ferrocast nunca escribe el archivo por su cuenta: se lo pide a Obsidian mediante un enlace `obsidian://`, así que se aplican los ajustes de tu vault (carpeta por defecto, plantillas, formato de fecha).

| Comando | Qué hace |
|---|---|
| **New Obsidian Note** | Un formulario con título y texto. <kbd>⌘</kbd> <kbd>↵</kbd> crea la nota y la abre en Obsidian. Se guarda donde indique «Default location for new notes» de tu vault. |
| **Append to Daily Note** | Un formulario con el texto. <kbd>⌘</kbd> <kbd>↵</kbd> lo añade al final de la nota diaria de hoy y Obsidian sigue en segundo plano. Se añade un salto de línea tras tu texto. |
| **Add Clipboard to Daily Note** | Añade el texto del portapapeles a la nota diaria de hoy, sin formulario. |

Si el título está vacío, Ferrocast usa la primera línea del texto (hasta 80 caracteres) o, si no hay texto, la fecha y la hora (`2026-10-22 18.30`). Los caracteres que no pueden ir en un nombre de archivo (`/ \ : * ? " < > |` y `# ^ [ ]`) se sustituyen por un espacio. En el formulario, <kbd>↵</kbd> en el título pasa al texto y <kbd>esc</kbd> vuelve atrás.

La nota diaria necesita el complemento básico **Daily notes** de Obsidian (Ajustes → Core plugins). Si Ferrocast puede leer que está desactivado, te lo dice y no envía nada. Cada comando comprueba, por este orden: que hay un vault elegido, que la carpeta existe, que Obsidian está instalado y, en la nota diaria, que el complemento está activo. Si falla una comprobación, recibes un mensaje, no se abre nada y el texto que escribiste se queda en el formulario. Un enlace de más de 8.000 caracteres se rechaza en lugar de recortarse.

## Qué se lee

- Ferrocast lee tu vault en tu Mac. No se sube nada y no se hace ninguna petición de red.
- El índice guarda la ruta, el título, los alias y la fecha de modificación de cada nota. Vive en memoria y se reconstruye cada vez que abres el comando.
- La búsqueda dentro de las notas lee los archivos del disco, con un límite de lectura por búsqueda.
- No se cambia, mueve ni borra nada de tu vault.
- Si tu vault está en una carpeta sincronizada en la nube (iCloud Drive, Dropbox, OneDrive), la primera indexación de una sesión puede tardar varios segundos mientras responde el proveedor. La ventana sigue respondiendo. Las notas que el proveedor aún no ha descargado se excluyen de la búsqueda dentro de las notas, de modo que una búsqueda nunca inicia descargas masivas. Se siguen encontrando por título. Al seleccionar una de esas notas, la vista previa hace que el proveedor descargue ese único archivo.

## Solución de problemas

| Mensaje o síntoma | Qué hacer |
|---|---|
| **The vault folder was not found** | La carpeta se movió, cambió de nombre o está en un disco que no está conectado. Elige el vault de nuevo en Ajustes → Integrations. |
| **Choose your Obsidian vault** | Todavía no hay un vault guardado. Abre Ajustes → Integrations y elige uno. |
| **Obsidian is not installed. Download it from obsidian.md.** | Instala Obsidian. La búsqueda, la vista previa, Reveal in Finder y las acciones de copiar siguen funcionando sin él. |
| Una nota no aparece | Se omiten las carpetas que empiezan por punto (`.obsidian`, `.trash`, `.git`), solo se indexan archivos `.md`, se omiten los archivos de más de 2 MB y un vault se indexa hasta 50.000 notas. |
| No se encuentra una palabra dentro de una nota | La búsqueda dentro de las notas empieza a los 3 caracteres, omite las notas que aún son marcadores de la nube y se detiene tras leer una cantidad fija de texto. Busca por título o usa **Search in Obsidian**. |
| **That link is too long for Obsidian (over 8,000 characters).** | Acorta el texto que estás buscando. |
| **Could not read this note.** | El archivo se borró, se movió o no se puede leer. Vuelve a abrir el comando para actualizar el índice. |
| **Choose your Obsidian vault in Settings → Integrations.** | Usaste un comando de creación antes de elegir un vault. Se abre la pestaña; elige el vault y vuelve a enviar. |
| **Turn on the Daily notes plugin in Obsidian (Settings → Core plugins).** | Activa el complemento básico en Obsidian y vuelve a enviar. Tu texto sigue en el formulario. |
| **That is too long to send through Obsidian.** | Las notas nuevas y las entradas diarias viajan dentro de un enlace. Acorta el texto o pégalo directamente en Obsidian. |
| **The clipboard has no text.** | **Add Clipboard to Daily Note** solo envía texto. Copia primero algo de texto. |
