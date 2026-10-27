---
title: "Migrar desde Raycast"
description: "Qué se conserva al pasar de Raycast a Ferrocast. Los Script Commands funcionan tal cual, los snippets se importan desde JSON y qué aún no está incluido."
sidebar:
  order: 4
---
Ferrocast es independiente y no está afiliado a Raycast.

## Qué se conserva

- Los **Script Commands** funcionan tal cual. Ferrocast lee los metadatos compatibles con Raycast y admite los modos full, compact, silent e inline.
- Los **snippets** se importan desde una exportación JSON de Raycast (ajustes, copia de seguridad e importación).
- Los **quicklinks** los recreas a mano; lleva un minuto.

## Qué no está incluido

Extensiones React, sincronización, Search Menu Bar Items y contexto de IA (selección o pantalla). No hay tienda de extensiones.

## Límites

:::caution[Límites]
Los argumentos de los Script Commands se pasan por línea de comandos, como en Raycast, y pueden verse con `ps` mientras el script corre.
:::

## ¿Comparando opciones?

La página [Alternativa a Raycast](/es/alternativa-a-raycast/) resume qué conservas, qué falta y una tabla con fuentes de Ferrocast, Raycast y Alfred.

## Relacionado

- [Script Commands](/es/docs/launcher/script-commands/)
- [Preguntas frecuentes](/es/docs/reference/faq/)
