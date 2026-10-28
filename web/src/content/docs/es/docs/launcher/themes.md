---
title: "Temas personalizados"
description: "Crea tus propios colores de Ferrocast en Ajustes → General: acento, fondo, superficie y texto, con vista previa, avisos de contraste y exportación o importación como archivo."
sidebar:
  order: 13
---
Además de System, Dark y Light, Ferrocast puede usar los colores que elijas. Un tema personalizado es un conjunto de colores guardado sobre una base clara u oscura. El resto (hover, filas seleccionadas, bordes, texto de acento legible) se calcula a partir de ellos.

## Crear un tema

1. Abre Ajustes → General (<kbd>⌘</kbd> <kbd>,</kbd>).
2. En **Custom themes**, pulsa **New from current**. Ferrocast copia el aspecto en uso (oscuro o claro) en un tema llamado "Dark copy" o "Light copy" y lo selecciona.
3. Cambia los colores. Cada uno tiene un selector de color y un campo hexadecimal (`#RRGGBB`; también vale `#RGB`). El lanzador, Ajustes y las ventanas de herramientas se repintan mientras editas, y un lanzador pequeño bajo el editor muestra el resultado.

El editor tiene estos valores:

| Valor | Para qué sirve |
| --- | --- |
| Base | Clara u oscura. Define cómo se derivan los colores de hover y selección y si la ventana de Notes es clara u oscura. |
| Accent (acento) | Botones, el cursor de texto, resaltados e interruptores activados. |
| Background (fondo) | La ventana. |
| Surface (superficie) | Tarjetas, campos y controles. |
| Text (texto) | Títulos y resultados. |
| Secondary text | Descripciones y pistas. |
| Separator | Líneas entre filas. |

Los temas System, Dark y Light vienen incluidos y no se pueden editar. **New from current** los copia en un tema que sí puedes editar. Con un tema personalizado seleccionado, el botón pasa a **Duplicate** y copia ese tema.

## Elegir, renombrar, eliminar

Usa la lista desplegable de **Custom themes** para cambiar de tema. Elige **None** o pulsa System, Dark o Light en **Appearance** para volver a un aspecto incluido. **Rename** cambia el nombre (los nombres son únicos, sin distinguir mayúsculas). **Delete** elimina el tema seleccionado.

## Avisos de contraste

El texto debe seguir siendo legible. Cuando el texto sobre el fondo, el texto sobre la superficie o el texto secundario sobre el fondo queda por debajo de una relación de contraste de 4,5:1 (WCAG AA para texto normal), un aviso bajo el editor indica el par y su relación. Es solo un aviso: puedes conservar el tema.

## Compartir un tema

**Export…** guarda el tema seleccionado como archivo `.ferrocast-theme` (JSON con un nombre de formato y una versión). **Import…** lee uno; si el nombre ya existe, el tema importado recibe un número ("Ocean 2"). Los archivos de una versión de formato más reciente, los que no son temas y los que tienen un color no válido se rechazan con un mensaje.

Los temas se guardan en `themes.json`, en la carpeta de datos; el tema seleccionado se guarda en `settings.json`.

:::caution[Límites]
- Los colores que tienen significado no cambian: los indicadores de estado en verde, ámbar y rojo, la superficie de vista previa del vídeo en el editor de grabaciones y las diapositivas del tour de bienvenida.
- La ventana de Notes sigue la base del tema (clara u oscura), no sus colores individuales.
- Ajustes → General no tiene una fila buscable para cada color; busca "Custom themes".
:::

## Relacionado

- [Ajustes y copia de seguridad](/es/docs/launcher/settings/)
- [Notes](/es/docs/launcher/notes/)
