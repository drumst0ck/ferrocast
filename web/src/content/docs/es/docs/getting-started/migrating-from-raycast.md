---
title: "Migrar desde Raycast"
description: "Qué se conserva de Raycast: Script Commands, snippets, extensiones ya instaladas (en parte) y sincronización opcional con iCloud."
sidebar:
  order: 4
---
Ferrocast es independiente y no está afiliado a Raycast.

## Qué se conserva

- Los **Script Commands** funcionan tal cual. Ferrocast lee los metadatos compatibles con Raycast y admite los modos full, compact, silent e inline.
- Los **snippets** se importan desde una exportación JSON de Raycast (ajustes, copia de seguridad e importación).
- Los **quicklinks** los recreas a mano; lleva un minuto.
- Las extensiones que ya instalaste con Raycast pueden ejecutarse si activas [Ejecutar extensiones de Raycast](/es/docs/launcher/raycast-extensions/). Está desactivado por defecto, necesita Node.js 22 o posterior y no contacta con la Store de Raycast. No todas las partes funcionan.
- [Sincronizar con iCloud](/es/docs/launcher/sync/) es opcional (Ajustes → Advanced, desactivado por defecto). No es la Cloud Sync de Raycast.

## Qué no está incluido

Una tienda de extensiones propia, Search Menu Bar Items y contexto de IA (selección o pantalla). Aún no están soportados los comandos de la barra de menús, el inicio de sesión OAuth, las funciones de IA, el selector de archivos y el accesorio de enlace.

## Límites

:::caution[Límites]
Los argumentos de los Script Commands se pasan por línea de comandos, como en Raycast, y pueden verse con `ps` mientras el script corre.
:::

## ¿Comparando opciones?

La página [Alternativa a Raycast](/es/alternativa-a-raycast/) resume qué conservas, qué falta y una tabla con fuentes de Ferrocast, Raycast y Alfred.

## Relacionado

- [Script Commands](/es/docs/launcher/script-commands/)
- [Preguntas frecuentes](/es/docs/reference/faq/)
