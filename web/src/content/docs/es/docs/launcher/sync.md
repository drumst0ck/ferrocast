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

Ferrocast fusiona elemento por elemento y gana la edición más reciente. Si dos Mac editan la misma nota, el texto más antiguo no se descarta: se guarda como una nota nueva que termina en "(copia en conflicto de <nombre del Mac>)" y Ferrocast muestra un aviso. Una edición hecha después de recibir la versión de otro Mac gana aunque el reloj de este Mac vaya atrasado.

- Cada snippet, quicklink, tema propio, nota, entrada del portapapeles y chat es un elemento propio.
- **Cada ajuste es un elemento propio** (y cada ajuste anidado, como una opción de captura o las opciones de un comando). Si un Mac cambia el tema y otro cambia el atajo, se conservan ambos cambios.
- Un elemento nuevo en un Mac nunca se pierde porque otro Mac no lo tenga.
- Un borrado llega a tus otros Mac. Si otro Mac edita el elemento después del borrado, el elemento vuelve.
- Un borrado se recuerda hasta que todos los Mac que siguen en uso lo han visto (mira Macs y limpieza más abajo), así que mientras tanto un Mac antiguo o una carpeta sobrante en iCloud Drive no puede devolver un elemento borrado.
- Los borrados del portapapeles se recuerdan 400 días (los 3000 más recientes), más que el ajuste de historial más largo. Una entrada fijada del portapapeles de más de 400 días no se sincroniza, así que las ediciones sobre ella se quedan en el Mac donde las hiciste.
- Cada Mac escribe solo sus propios archivos en la carpeta `Ferrocast` de iCloud Drive (`devices/<id>/`), así que iCloud no tiene copias en conflicto que crear. Un archivo que Ferrocast no pueda leer se omite.

## Documentos del portapapeles

El documento del portapapeles de cada Mac contiene solo las entradas copiadas en ese Mac, así que su tamaño en iCloud Drive no crece con el número de Mac. Cada Mac sigue teniendo todo el historial, porque lee el documento de todos los demás.

- Una entrada que copiaste en un Mac la publica ese Mac. Un Mac que solo la adoptó no la vuelve a publicar, salvo que ningún otro Mac la tenga (por ejemplo, porque el Mac que la copió se olvidó): entonces la sigue publicando el Mac con el id más bajo.
- Si editas una entrada desde otro Mac (por ejemplo, la fijas), la publica el Mac que la editó. Si la borras, ese Mac publica el borrado, que llega a todos los Mac.
- Ferrocast v0.7.0 y anteriores combinan todos los documentos, así que siguen funcionando con los nuevos. Sus documentos aún contienen las entradas que adoptaron; con esta versión se reducen a las entradas propias del Mac en la primera sincronización.
- Si un Mac pierde su historial del portapapeles, lo recupera desde su propio documento y desde los de los demás Mac.
- Un Mac solo sube las imágenes de las entradas que publica.

## Macs y limpieza

Ajustes → Advanced → Sync muestra los Mac que han publicado datos de sincronización: el nombre que publicó cada uno y cuándo se vio por última vez. **Forget this Mac** (Olvidar este Mac), tras una confirmación, elimina de la carpeta de sincronización la carpeta de ese Mac (`devices/<id>/`: sus documentos e imágenes) y este Mac deja de leerla. No se ofrece para el Mac que estás usando, y Ferrocast nunca elimina la carpeta de este Mac ni la de otro. Si el Mac olvidado vuelve a sincronizar, publica de nuevo y reaparece en la lista. Úsalo para un Mac que retiraste o reinstalaste, para que deje de ocupar espacio.

Los borrados (tombstones) también se limpian. Un borrado se elimina cuando todos los Mac que siguen activos han publicado un documento posterior, es decir, cuando todos lo han visto. Un Mac está activo si publicó en los últimos 30 días y no se ha olvidado. Un borrado nunca se elimina antes, y los snippets y quicklinks integrados que borraste conservan su borrado. Detalles:

- Un Mac activo con un Ferrocast antiguo (no publica hora de ejecución), o cuyo documento no se puede leer o aún se está descargando, retrasa la limpieza hasta que se actualice, se olvide o lleve 30 días sin conexión. Si no hay otro Mac activo, no se elimina nada.
- Un Mac que estuvo más de 30 días sin conexión ya no retrasa la limpieza, y mientras otro Mac esté activo no se leen sus documentos antiguos. Al volver, una nota, snippet o quicklink que aún tiene sin cambios y que los demás Mac borraron y limpiaron se elimina también en él, en vez de reaparecer; lo que editaste en él mientras estaba fuera se conserva y se comparte de nuevo. Esto solo ocurre si en esa ejecución se leyeron sin problemas los documentos de todos los demás Mac activos.
- Las entradas del portapapeles no se limpian así: un borrado del portapapeles hecho por un Mac que luego se olvidó, o que lleva mucho tiempo sin conexión, deja de proteger frente a un tercer Mac que también estuvo fuera. Ese Mac puede recuperar la entrada para sí cuando vuelva.
- Ferrocast actualiza la hora de ejecución de un documento cada pocas horas, no en cada ejecución, así que un borrado puede esperar unas horas más de lo estrictamente necesario.
- Los borrados del portapapeles mantienen su propia regla (400 días, los 3000 más recientes).

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
