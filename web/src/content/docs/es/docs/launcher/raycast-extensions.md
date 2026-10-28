---
title: "Ejecutar extensiones de Raycast"
description: "Ejecuta en Ferrocast las extensiones que instalaste con Raycast. Desactivado por defecto; pide Node.js 22 o superior. Qué funciona, qué no y los riesgos."
sidebar:
  order: 16
---
Ferrocast ejecuta extensiones de Raycast: las que ya instalaste con Raycast aparecen en el lanzador junto a los comandos propios de Ferrocast. Las lee de `~/.config/raycast/extensions` y ejecuta su código con una implementación propia de la API de extensiones de Raycast. No contacta con la Store de Raycast y no instala nada por ti.

Raycast es una marca de su propietario. Ferrocast no está afiliado, respaldado ni patrocinado por Raycast.

## Actívalo

Está **desactivado por defecto**. Mientras no lo actives, Ferrocast no lee la carpeta de extensiones ni inicia código de ninguna extensión.

1. Abre Ajustes → Integrations y busca el grupo **Raycast extensions**.
2. Activa **Run Raycast extensions**.
3. Mira la fila **Node.js 22 or newer**. Si dice **Ready**, ya está.
4. Cada extensión encontrada en la carpeta aparece con su propio interruptor. Una extensión desactivada no tiene comandos en el lanzador y no se puede iniciar.

Los comandos aparecen en el lanzador con su propio nombre y el de la extensión como subtítulo. Si aún no hay extensiones en la carpeta, la lista lo indica; instala algunas con Raycast primero.

Estos interruptores no se sincronizan: Node.js y las extensiones instaladas existen en cada Mac, así que [la sincronización con iCloud](/es/docs/launcher/sync/) no los incluye.

## Node.js

Las extensiones son JavaScript, así que Ferrocast necesita Node.js 22 o superior. Primero busca uno en tu Mac (por ejemplo el de Homebrew). Si no encuentra ninguno, **Download Node.js (about 26 MB)** descarga la versión oficial de nodejs.org. Solo ocurre cuando pulsas el botón. Ferrocast comprueba el archivo con una suma SHA-256 incluida en la app, rechaza lo que no coincida y conserva solo el programa `node` en su propia carpeta de datos, en `~/Library/Application Support/Ferrocast/runtime`. No se instala nada en el sistema. La app no incluye Node.js.

## Preferencias

Muchas extensiones tienen preferencias (una clave de API, una carpeta por defecto). Pulsa **Preferences…** en la fila de la extensión para editarlas en un formulario. Se admiten campos de texto, contraseñas, casillas y desplegables; las preferencias de archivo, carpeta y app se escriben como una ruta o un nombre. Las contraseñas se ocultan al escribir y nunca se escriben en el registro de Ferrocast.

Los valores se guardan en `extensions/<nombre>/preferences.json`, dentro de la carpeta de datos de Ferrocast, como texto plano en ese archivo (no en el Llavero). Si inicias un comando que necesita una preferencia obligatoria sin rellenar, Ferrocast abre este formulario en lugar de ejecutarlo.

## Qué funciona

Las extensiones se dibujan con la interfaz propia de Ferrocast, no con una vista web. Funcionan estas partes de la API:

- **Listas** con secciones, subtítulos, accesorios, búsqueda (también mientras escribes), panel de detalle y filtros desplegables, que aparecen como opciones en el panel de acciones (⌘K).
- Pantallas de **detalle** con Markdown.
- **Formularios** con texto, contraseña, área de texto, casilla, desplegable, fecha (escrita como `AAAA-MM-DD`) y etiquetas. ⌘↵ envía.
- **Acciones** y sus atajos, apilar y volver entre pantallas, avisos, copiar y pegar, abrir URL y archivos, mostrar un archivo en Finder.
- **Almacenamiento y datos**: LocalStorage, Cache, las preferencias, la lista de tus apps, la app activa, el texto seleccionado y los diálogos de confirmación.
- Las **cuadrículas** se muestran como listas, con el contenido de la celda como icono.

Para medirlo, ejecutamos 20 extensiones populares del repositorio público de Raycast (github.com/raycast/extensions, licencia MIT) que no necesitan cuenta, clave de API ni IA. Tomamos las 20 con más instalaciones según sus páginas de la Store, de entre unas 60 candidatas que no necesitan cuenta, clave ni IA. No son toda la Store. Tienen 40 comandos que abren una vista y manejamos cada uno sin la interfaz: lanzarlo, buscar, abrir la primera acción, apilar una pantalla y volver, enviar un formulario con sus valores por defecto. 34 de los 40 funcionaron, 4 solo esperaron datos que no les dimos (por ejemplo un portapapeles vacío), 1 falló porque Homebrew rechazó cargar un cask de un tap no confiable y 1 mostró un historial vacío. La prueba no demuestra que todos los comandos de todas las extensiones funcionen y no usó la ventana de la app. Tus extensiones pueden comportarse distinto.

## Qué no funciona todavía

- **Comandos de la barra de menús**: no aparecen en la lista.
- **Campos de selector de archivos** en formularios y el accesorio de enlace.
- **Inicio de sesión con OAuth** y las funciones de IA de Raycast.
- **Algunas acciones**: mover a la Papelera, abrir con, crear un snippet o un quicklink y elegir una fecha.
- **Lanzar un comando de otra extensión**, y todo lo que dependa de la cuenta, la sincronización o la Store de Raycast.
- El **Markdown del panel de detalle en listas** se muestra como texto plano.

Cuando una extensión usa algo que Ferrocast no puede dibujar, la pantalla dice "Not supported in Ferrocast yet: ", el nombre de la parte, la extensión y un enlace a esta página. El resto de la extensión sigue funcionando donde puede.

## Seguridad y privacidad

**Las extensiones son programas escritos por otras personas. Se ejecutan en tu Mac con tus permisos.** Una extensión puede leer tus archivos, usar la red, ver lo que copias, leer el texto que has seleccionado en otras apps y ver la lista de apps que usas. Actívalo solo para extensiones que instalaste y en las que confías.

Lo que hace Ferrocast para limitar el daño:

- Cada comando se ejecuta en su propio proceso de Node.js, que se detiene al salir del comando. Las llamadas por encima de 8 en curso y `launchCommand` por encima de 5 cada 10 segundos se rechazan con un error, y stderr se corta a las 500 líneas. Un comando se detiene si escribe una línea de stdout de más de 16 MB, si su cola de entrada se llena, si no dibuja nada en 30 segundos, si un comando sin pantalla tarda 60 segundos o si lleva 90 segundos oculto.
- El proceso no recibe tus variables de entorno, solo una lista corta que los programas necesitan para funcionar.
- Las llamadas propias de Ferrocast para abrir y para mostrar en Finder se comprueban. Una extensión sigue ejecutándose con todos tus permisos de usuario y puede hacer más por su cuenta.
- Un comando con pantalla solo recibe el texto seleccionado y los diálogos mientras esa pantalla está visible; un comando sin pantalla los conserva mientras está oculto.
- Lo que una extensión escribe en stderr puede aparecer en el registro de Ferrocast. Ferrocast registra los nombres de las preferencias guardadas, nunca sus valores.
- Un proceso que se desvincula por su cuenta (por ejemplo con `setsid`) puede sobrevivir a la detención.

Nada de esto hace segura una extensión en la que no confías. Ferrocast no envía a ningún sitio tus extensiones, preferencias ni datos; lo que una extensión envíe por la red por su cuenta depende de su código.
