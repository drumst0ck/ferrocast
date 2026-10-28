---
title: "Sincronizar con iCloud"
description: "Sincroniza ajustes, snippets, quicklinks, notas con sus imágenes, portapapeles, chats y temas entre tus Mac con iCloud Drive."
sidebar:
  order: 20
---
La sincronización mantiene los datos de Ferrocast iguales en cada Mac que uses. Funciona a través de la carpeta de iCloud Drive, así que necesitas tener iCloud Drive activado en Ajustes del Sistema. Está **desactivada por defecto**: no se lee ni se escribe nada en iCloud Drive hasta que la actives.

Actívala en Ajustes → Advanced → Sync with iCloud. La línea de estado bajo **Sync now** muestra la hora de la última sincronización y cuántos otros Mac ha visto Ferrocast, o el motivo por el que no puede ejecutarse (por ejemplo, que falta la carpeta de iCloud Drive). **Sync now** sincroniza al instante.

## Qué se sincroniza

- Los **ajustes**, incluidos los atajos de teclado.
- Los **snippets** y los **quicklinks**.
- Los **temas propios**. Cada tema es un elemento, así que un tema que edites, renombres o borres llega a tus otros Mac. Si renombras el tema seleccionado en un Mac, los demás lo siguen teniendo seleccionado. Dos temas con el mismo nombre creados en Macs distintos se conservan los dos; renombra uno.
- Las **notas**, incluidas las fijadas y las de Eliminadas recientemente, y las imágenes que contienen (ver abajo).
- El **historial del portapapeles**, con las imágenes de las entradas de imagen. Solo se comparte lo que Ferrocast grabó en un Mac. Las entradas de gestores de contraseñas nunca se graban, así que nunca se comparten. Se sigue aplicando tu ajuste de retención del historial.
- Los **chats de Ask AI**. Ferrocast ahora guarda cada conversación para que pueda llegar a tus otros Mac.

## Qué se queda en cada Mac

- Las carpetas de scripts, la bóveda de Obsidian, Launch at login, la cámara y el micrófono que elegiste para grabar, las carpetas de capturas y de grabaciones, las preferencias y el estado de las actualizaciones, el interruptor del mosaico automático y la disposición de cada pantalla (los demás ajustes del mosaico sí se sincronizan), y el estado de ventanas y notas, como la última nota abierta y el zoom de Notas.
- Los interruptores de las extensiones de Raycast: ejecutar extensiones de Raycast y el interruptor de cada extensión. Las preferencias y los datos de las extensiones también se quedan en cada Mac.
- El propio interruptor de sincronización: lo activas por separado en cada Mac.
- Los **permisos de macOS** (Accesibilidad, Grabación de pantalla, Monitorización de entrada, etc.). Pertenecen a cada Mac y Ferrocast no puede copiarlos. Concédelos de nuevo en cada Mac.
- Las **claves de API** del llavero. Ferrocast está firmada ad hoc, lo que no permite el llavero de iCloud, así que introduce cada clave en cada Mac.

## Imágenes de las notas

Las imágenes de las notas viajan como archivos aparte, junto a los registros de las notas, en la misma carpeta `Ferrocast` de iCloud Drive.

- Cada Mac escribe solo su propia carpeta, `devices/<id>/attachments/`, así que dos Mac nunca escriben el mismo archivo. Una imagen se sube una sola vez, desde el Mac que la tiene, y solo cuando una nota la enlaza. Un Mac que descargó una imagen no la vuelve a subir.
- Cuando una nota sincronizada enlaza una imagen que este Mac no tiene, Ferrocast la copia de la carpeta del otro Mac, tras comprobar que el archivo está completo y es el que nombra la nota. Hasta que llega (iCloud puede estar descargándola) la nota muestra un marcador, el estado de la sincronización cuenta el archivo y la imagen aparece cuando ya está aquí.
- Una imagen nunca se elimina de iCloud Drive mientras las notas de algún Mac la enlacen, y un Mac nunca elimina ninguna mientras las notas de otro Mac no se puedan leer. Una imagen que nada enlaza se elimina 7 días después de que desapareciera el último enlace.
- Cada imagen pesa como máximo 20 MB. Al desactivar la sincronización se detiene todo el copiado; no se sube ni se elimina nada.

## Conflictos

Ferrocast fusiona elemento por elemento y gana la edición más reciente.

- Cada snippet, quicklink, tema propio, nota, entrada del portapapeles y chat es un elemento propio.
- **Cada ajuste es un elemento propio** (y cada ajuste anidado, como una opción de captura o las opciones de un comando). Si un Mac cambia el tema y otro cambia el atajo, se conservan ambos cambios.
- Un elemento nuevo en un Mac nunca se pierde porque otro Mac no lo tenga.
- Un borrado llega a tus otros Mac. Si otro Mac edita el elemento después del borrado, el elemento vuelve.
- Un borrado se recuerda para siempre, así que un Mac antiguo o una carpeta sobrante en iCloud Drive no puede devolver un elemento borrado.
- Los borrados del portapapeles se recuerdan 400 días (los 3000 más recientes), más que el ajuste de historial más largo. Una entrada fijada del portapapeles de más de 400 días no se sincroniza, así que las ediciones sobre ella se quedan en el Mac donde las hiciste.
- Cada Mac escribe solo sus propios archivos en la carpeta `Ferrocast` de iCloud Drive (`devices/<id>/`), así que iCloud no tiene copias en conflicto que crear. Un archivo que Ferrocast no pueda leer se omite.

## Activar la sincronización por primera vez

- Lo que un Mac ya tiene cuenta como la versión más antigua, aunque iCloud todavía no haya mostrado los archivos de los otros Mac. Un Mac que se une después adopta la versión más reciente de cada elemento que tienen tus otros Mac, y lo que solo tiene él se añade.
- Un elemento que nunca cambiaste (un ajuste, snippet o quicklink tal como venía con Ferrocast) cuenta como sin modificar. Una versión que cambiaste en cualquier Mac gana a esa sea cual sea la hora, así que un Mac nuevo que sincronizó primero no puede borrar las personalizaciones de tu Mac antiguo. Volver a poner un valor por defecto a propósito es una edición como cualquier otra.
- Un snippet o quicklink integrado que borraste antes de activar la sincronización sigue borrado cuando añades un Mac.
- El Mac que sincronizó primero conserva sus datos. Tras su segunda sincronización (unos 30 segundos) sus elementos cuentan como hechos en su primera sincronización, lo que gana a cualquier Mac que se una después. Los elementos que un Mac solo tomó de otro Mac no reciben esa hora, así que una edición hecha en otro sitio entre tanto sigue ganando.
- Si dos Mac activan la sincronización por primera vez antes de que alguno haya sincronizado dos veces, con versiones distintas del mismo elemento, gana en ambos el de id de dispositivo mayor, así que terminan iguales.
- Ferrocast guarda una copia de los ajustes de este Mac como `settings.before-sync.json` en la carpeta de datos antes de que la primera fusión los cambie.
- Si se pierde el estado de sincronización de un Mac, Ferrocast lo reconstruye a partir del propio archivo de ese Mac en iCloud Drive, así que se conservan las ediciones y los borrados hechos desde su última sincronización.
- Si un archivo de datos de un Mac no se puede leer (Ferrocast lo guarda junto al original como `.corrupt`), ese Mac parte de su propio archivo en iCloud Drive y de lo que tienen tus otros Mac, en lugar de decirles que se borró todo. Lo mismo ocurre si Ferrocast no puede leer el archivo en absoluto. La línea de estado lo indica.

## Cuándo se sincroniza

- Cada 30 segundos, al abrir el launcher y al pulsar **Sync now**.
- Mientras escribes en la ventana de Notas, las notas fusionadas esperan hasta que haces una pausa de dos segundos.
- Al desactivarla se detiene toda lectura y escritura, pero la carpeta queda en iCloud Drive tal como está.

:::note[Historial del portapapeles]
Con la sincronización activada, el historial sale del Mac y en iCloud Drive solo tiene el cifrado que ofrece el propio iCloud. Déjala desactivada si no quieres eso.
:::
