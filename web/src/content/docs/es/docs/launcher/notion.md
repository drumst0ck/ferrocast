---
title: "Buscar en Notion"
description: "Busca las páginas y bases de datos de Notion que compartes con Ferrocast, ábrelas en Notion y copia sus enlaces. Conecta con un token de integración."
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

## Privacidad

- El token se guarda en el llavero de macOS. Nunca se escribe en `settings.json`, en los registros ni en la línea de comandos de ningún proceso.
- Ferrocast solo se comunica con `api.notion.com`, y únicamente cuando abres Search Notion, escribes en él o usas los botones de Ajustes → Integrations.
- Lo que escribes en el cuadro de búsqueda y los títulos de las páginas que devuelve Notion viajan hacia y desde Notion. No se envía nada más.
- Search Notion lee y abre páginas. Nunca modifica ni borra nada de tu espacio de trabajo.

## Solución de problemas

| Mensaje | Qué hacer |
|---|---|
| **Notion rejected the token. Paste a new one in Settings → Integrations.** (401) | El token se revocó o se escribió mal. Cópialo de nuevo desde notion.so/my-integrations y pégalo en Ajustes → Integrations. |
| **Notion cannot see that page. Share it with your integration (••• → Connections).** | La página no está compartida con la integración. En Notion elige **•••** → **Conexiones** y añádela. |
| **The Notion integration lacks permission for this.** | A la integración le falta una capacidad o el acceso a esa página. Revisa sus capacidades en notion.so/my-integrations y añádela a la página en **Conexiones**. |
| **Notion is rate limiting requests. Try again in a few seconds.** | Notion admite solo unas pocas peticiones por segundo. Espera un momento y vuelve a buscar. |
| **Could not reach Notion. Check your connection.** | No tienes conexión o algo bloquea `api.notion.com`. La lista se queda como estaba, así que puedes volver a intentarlo. |
| No aparece nada | Comprueba que has compartido al menos una página y pulsa **Test connection** en Ajustes → Integrations. |
