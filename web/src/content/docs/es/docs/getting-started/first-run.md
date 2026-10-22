---
title: "Primer arranque y permisos"
description: "Qué pide Ferrocast la primera vez que lo abres, por qué necesita Accesibilidad, Input Monitoring y Automatización de Finder, y qué funciona sin ellos."
sidebar:
  order: 2
---
La primera vez que abres Ferrocast muestra un **recorrido de bienvenida** de siete diapositivas: qué hace la app y, después, una diapositiva que te guía por los permisos que puede usar. Usa <kbd>←</kbd> <kbd>→</kbd> o <kbd>↵</kbd> para avanzar y <kbd>esc</kbd> para saltarlo. Busca «Welcome» para volver a verlo. Los permisos los concedes tú en Ajustes del Sistema; Ferrocast no puede concederlos por sí mismo.

Abre el lanzador con <kbd>⌥</kbd> <kbd>Space</kbd> (se cambia en los ajustes).

## Permisos

| Permiso | Para qué se necesita |
|---|---|
| Accesibilidad | Mover ventanas, pegar por ti, expandir snippets, hotkeys globales. |
| Input Monitoring | Funciones que observan el teclado, como la auto-expansión de snippets. macOS puede pedirlo junto con Accesibilidad. |
| Automatización de Finder | El comando Convert Finder Selection, que lee lo seleccionado en Finder. |

La búsqueda, la calculadora y la IA funcionan sin Accesibilidad. La rueda al arrastrar tampoco necesita el permiso de Accesibilidad.

## Permisos y firma

Los permisos (Accesibilidad / Input Monitoring) los concede el usuario. Al recompilar con firma ad-hoc macOS puede pedirlos de nuevo. Para distribuir hace falta firma Developer ID y notarización.

## Relacionado

- [Instalar](/es/docs/getting-started/install/)
- [Gestión de ventanas](/es/docs/launcher/window-management/)
- [Snippets](/es/docs/launcher/snippets/)
- [Rueda al arrastrar](/es/docs/files/drag-wheel/)
