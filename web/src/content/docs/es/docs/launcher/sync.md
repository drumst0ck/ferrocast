---
title: "Sincronizar con iCloud"
description: "Mantén los ajustes, snippets, quicklinks, notas, historial del portapapeles y chats de Ask AI de Ferrocast iguales entre tus Mac a través de iCloud Drive. Desactivado por defecto."
sidebar:
  order: 20
---
La sincronización mantiene los datos de Ferrocast iguales en cada Mac que uses. Funciona a través de la carpeta de iCloud Drive, así que necesitas tener iCloud Drive activado en Ajustes del Sistema. Está **desactivada por defecto**: no se lee ni se escribe nada en iCloud Drive hasta que la actives.

Actívala en Ajustes → Advanced → Sync with iCloud. La línea de estado bajo **Sync now** muestra la hora de la última sincronización y cuántos otros Mac ha visto Ferrocast, o el motivo por el que no puede ejecutarse (por ejemplo, que falta la carpeta de iCloud Drive). **Sync now** sincroniza al instante.

## Qué se sincroniza

- Los **ajustes**, incluidos los atajos de teclado.
- Los **snippets** y los **quicklinks**.
- Las **notas**, incluidas las fijadas y las de Eliminadas recientemente.
- El **historial del portapapeles**, con las imágenes de las entradas de imagen. Solo se comparte lo que Ferrocast grabó en un Mac. Las entradas de gestores de contraseñas nunca se graban, así que nunca se comparten. Se sigue aplicando tu ajuste de retención del historial.
- Los **chats de Ask AI**. Ferrocast ahora guarda cada conversación para que pueda llegar a tus otros Mac.

## Qué se queda en cada Mac

- Las carpetas de scripts, la bóveda de Obsidian, Launch at login, la cámara y el micrófono que elegiste para grabar, las carpetas de capturas y de grabaciones, las preferencias y el estado de las actualizaciones, y el estado de ventanas y notas, como la última nota abierta y el zoom de Notas.
- El propio interruptor de sincronización: lo activas por separado en cada Mac.
- Los **permisos de macOS** (Accesibilidad, Grabación de pantalla, Monitorización de entrada, etc.). Pertenecen a cada Mac y Ferrocast no puede copiarlos. Concédelos de nuevo en cada Mac.
- Las **claves de API** del llavero. Ferrocast está firmada ad hoc, lo que no permite el llavero de iCloud, así que introduce cada clave en cada Mac.

## Conflictos

Ferrocast fusiona elemento por elemento. Cada snippet, quicklink, nota, entrada del portapapeles y chat se compara por separado y gana la edición más reciente. Un elemento nuevo en un Mac nunca se pierde porque otro Mac no lo tenga. Un borrado también llega a tus otros Mac; si otro Mac edita el elemento después del borrado, el elemento vuelve. Los ajustes son un solo elemento: gana el cambio más reciente en cualquier Mac.

Cada Mac escribe solo sus propios archivos en la carpeta `Ferrocast` de iCloud Drive (`devices/<id>/`), así que iCloud no tiene copias en conflicto que crear. Un archivo de otro Mac que Ferrocast no pueda leer se omite.

La primera vez que activas la sincronización en un Mac, lo que ya tiene cuenta como la versión más antigua de cada elemento, aunque iCloud todavía no haya mostrado los archivos de los otros Mac. Un Mac que se une después adopta, por tanto, los ajustes y la versión más reciente de cada elemento que tienen tus otros Mac, y lo que solo tiene él se añade al conjunto. Si un Mac todavía tiene un ajuste o un snippet o quicklink integrado exactamente como venía con Ferrocast y nunca lo cambió, esa versión cuenta como sin modificar: una versión que personalizaste en otro Mac gana a esa sea cual sea la hora, así que añadir un Mac nuevo que sincronizó primero no puede borrar las personalizaciones de tu Mac antiguo. Volver a poner un valor en su valor por defecto a propósito cuenta como una edición más. El Mac que sincronizó primero conserva sus datos: tras su segunda sincronización (unos 30 segundos) sus elementos cuentan como hechos en su primera sincronización, lo que gana a cualquier Mac que se una después, sean cuales sean sus ids de dispositivo, y cualquier edición posterior sigue ganando por su hora. Un snippet o quicklink integrado que borraste antes de activar la sincronización sigue borrado cuando añades un Mac. Solo si dos Mac activan la sincronización por primera vez antes de que alguno haya sincronizado dos veces, con versiones distintas del mismo elemento, gana en ambos el de id de dispositivo mayor, así que terminan iguales. Un borrado se recuerda para siempre, así que un Mac antiguo o una carpeta sobrante en iCloud Drive no puede devolver un elemento borrado. Los borrados del portapapeles se recuerdan 400 días (se guardan como máximo los 3000 más recientes), más que el ajuste de historial más largo, y una entrada fijada del portapapeles de más de 400 días no se sincroniza, así que las ediciones sobre ella se quedan en el Mac donde las hiciste. Mientras escribes en la ventana de Notas, las notas fusionadas esperan hasta que haces una pausa de dos segundos. La fusión se hace cada 30 segundos, al abrir el launcher y al pulsar **Sync now**. Al desactivarla se detiene toda lectura y escritura, pero la carpeta queda en iCloud Drive tal como está.

:::note[Historial del portapapeles]
Con la sincronización activada, el historial sale del Mac y en iCloud Drive solo tiene el cifrado que ofrece el propio iCloud. Déjala desactivada si no quieres eso.
:::
