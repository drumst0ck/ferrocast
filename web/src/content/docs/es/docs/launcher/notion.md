---
title: "Notion: buscar y añadir"
description: "Busca las páginas de Notion que compartes con Ferrocast, ábrelas y añade texto o el portapapeles a una página o base de datos con un token de integración."
sidebar:
  order: 14
---
Search Notion encuentra tus páginas y bases de datos desde el lanzador y las abre en Notion. Usa un token de integración interna que creas tú mismo, así que no hay ventana de inicio de sesión ni ningún servidor de Ferrocast por en medio.

## Configuración

1. Abre [notion.so/my-integrations](https://www.notion.so/my-integrations) y crea una **integración interna** para tu espacio de trabajo.
2. Copia su token (empieza por `ntn_`).
3. En Ferrocast, abre Ajustes → Integrations, pega el token en **Notion token** y pulsa <kbd>↵</kbd>. **Test connection** muestra el nombre de la integración y del espacio de trabajo.
4. Comparte las páginas que quieras encontrar. En Notion, abre una página, elige **•••** → **Conexiones** y añade tu integración. Al compartir una página también se comparten las páginas que contiene.

Notion solo devuelve lo que se ha compartido con la integración. Si falta una página en los resultados, aún no está compartida.

## Buscar

Abre **Search Notion** desde el lanzador. Con el cuadro de búsqueda vacío muestra las 20 páginas y bases de datos editadas más recientemente. Escribe para buscar: Ferrocast espera un cuarto de segundo tras tu última tecla y entonces consulta a Notion, de modo que escribir rápido envía una petición y no una por letra.

Cada fila muestra el icono de la página (o uno genérico), el título, si es una **Page** o una **Database**, y cuándo se editó por última vez: «2h ago», «yesterday» o la fecha. Cuando Notion tiene más resultados, la última fila, **Load more**, añade los 20 siguientes. Mientras se ejecuta una petición el pie dice «Searching Notion» y puedes seguir escribiendo.

Sin token, la vista muestra **Connect Notion to search your pages** y una sola fila, **Open Settings → Integrations**, que te lleva al campo del token.

Puedes asignar al comando un alias o un atajo como a cualquier otro comando, desde <kbd>⌘</kbd> <kbd>K</kbd> en la raíz del lanzador.

## Acciones

Pulsa <kbd>⌘</kbd> <kbd>K</kbd> sobre un resultado.

| Acción | Atajo | Qué hace |
|---|---|---|
| **Open** | <kbd>↵</kbd> | Abre la página en Notion. Si la app de escritorio de Notion está instalada, gestiona el enlace; si no, lo abre tu navegador. |
| **Copy Link** | <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> | Copia la URL de la página. |
| **Set as Capture Target** | | Guarda la página o base de datos como destino de captura, la misma elección que Ajustes → Integrations → **Capture target**. |
| **Add Text Here…** | | Abre el formulario **Add to Notion** dirigido a este resultado para un único envío. El destino de captura guardado no cambia. |

## Add to Notion

Dos comandos más envían texto a tu **destino de captura**, la página o base de datos elegida en Ajustes → Integrations → **Capture target** (o con **Set as Capture Target**, arriba). Antes hay que conectar un token y elegir un destino. Sin token, el aviso dice «Connect Notion first: Settings → Integrations.» y abre esa pestaña; con token pero sin destino dice «Choose a capture target in Settings → Integrations.» y también la abre.

### Add to Notion (formulario)

Abre un formulario con un **Title** (opcional) y un cuadro de **Text** de varias líneas. El cursor empieza en el texto.

| Tecla | Qué hace |
|---|---|
| <kbd>↵</kbd> en **Title** | Pasa al cuadro de texto. |
| <kbd>⌘</kbd> <kbd>↵</kbd> | Envía. El pie muestra **Send ⌘↵**. |
| <kbd>esc</kbd> | Vuelve atrás. No se conserva nada. |

Cuando Notion acepta el texto, el aviso dice «Added to» seguido del nombre del destino y el formulario se vacía. Si el envío falla, el aviso explica qué ha pasado y el formulario conserva tu texto, para que puedas corregir el problema y pulsar de nuevo <kbd>⌘</kbd> <kbd>↵</kbd>.

### Add Clipboard to Notion

Envía de inmediato el texto del portapapeles, sin formulario, al mismo destino. Si el portapapeles está vacío o contiene algo que no es texto (una imagen o archivos), el aviso dice «The clipboard has no text.» y no se envía nada.

### Destino de página o de base de datos

- **Página**: el texto se añade al final de la página como Markdown. Si escribiste un título, va primero como encabezado `## Título`. El contenido existente nunca se modifica.
- **Base de datos**: Ferrocast crea una página nueva (una fila). Su nombre es el **Title** que escribiste; sin título es la primera línea no vacía del texto, recortada a 100 caracteres, y el cuerpo es el resto del texto. Si la primera línea tiene más de 100 caracteres, todo el texto queda en el cuerpo, de modo que el recorte no pierde nada. La columna de título de cada base de datos se lee una vez de Notion y se recuerda hasta que cierres Ferrocast.

El Markdown del texto (encabezados, listas, enlaces, bloques de código) se convierte en bloques de Notion.

### Texto muy largo

Un texto de más de 100 000 caracteres se divide en las líneas en blanco en partes consecutivas de como máximo 100 000 caracteres, que se envían una tras otra. Un bloque de código con vallas no se divide, salvo que por sí solo supere el límite; en ese caso se corta por líneas y la valla se cierra y se vuelve a abrir para que cada parte siga siendo válida. Si falla una parte posterior, el aviso indica cuántas partes se añadieron («Added 1 of 3 parts to …; the rest were not sent.») y tu texto se queda en el formulario. Enviarlo otra vez repetiría las partes ya añadidas, así que elimínalas antes o envía solo el resto.

Si Notion responde que está limitando las peticiones, Ferrocast espera un segundo y reintenta esa petición una vez.

## Privacidad

- El token se guarda en el llavero de macOS. Nunca se escribe en `settings.json`, en los registros ni en la línea de comandos de ningún proceso.
- Ferrocast solo se comunica con `api.notion.com`, y únicamente cuando abres Search Notion, escribes en él, envías con Add to Notion o Add Clipboard to Notion, o usas los botones de Ajustes → Integrations.
- Lo que escribes en el cuadro de búsqueda, los títulos de las páginas que devuelve Notion y el texto que decides añadir viajan hacia y desde Notion. No se envía nada más.
- Search Notion lee y abre páginas. Add to Notion solo añade: agrega contenido al final de una página o crea una página nueva en una base de datos, y nunca edita texto existente ni borra nada de tu espacio de trabajo.

## Solución de problemas

| Mensaje | Qué hacer |
|---|---|
| **Notion rejected the token. Paste a new one in Settings → Integrations.** (401) | El token se revocó o se escribió mal. Cópialo de nuevo desde notion.so/my-integrations y pégalo en Ajustes → Integrations. |
| **Notion cannot see that page. Share it with your integration (••• → Connections).** | La página no está compartida con la integración. En Notion elige **•••** → **Conexiones** y añádela. |
| **The Notion integration lacks permission for this.** | A la integración le falta una capacidad o el acceso a esa página. Revisa sus capacidades en notion.so/my-integrations y añádela a la página en **Conexiones**. |
| **Notion is rate limiting requests. Try again in a few seconds.** | Notion admite solo unas pocas peticiones por segundo. Espera un momento y vuelve a buscar. |
| **Could not reach Notion. Check your connection.** | No tienes conexión o algo bloquea `api.notion.com`. La lista se queda como estaba, así que puedes volver a intentarlo. |
| No aparece nada | Comprueba que has compartido al menos una página y pulsa **Test connection** en Ajustes → Integrations. |
| **Connect Notion first: Settings → Integrations.** | Todavía no hay un token guardado. Pega uno en Ajustes → Integrations. |
| **Choose a capture target in Settings → Integrations.** | Elige la página o base de datos en **Capture target**, o usa **Add Text Here…** en un resultado de la búsqueda. |
| **The clipboard has no text.** | Copia primero algún texto. No se pueden enviar imágenes ni archivos. |
| «Added 2 of 3 parts … the rest were not sent.» | Un texto largo se detuvo a la mitad. Tu texto sigue en el formulario; elimina la parte que ya está en Notion, o pega solo el resto, y vuelve a enviar. |
| La página se añade pero el título no es el esperado | En una base de datos, la primera línea es el título salvo que rellenes **Title**. Escribe un título en el formulario para elegirlo. |
