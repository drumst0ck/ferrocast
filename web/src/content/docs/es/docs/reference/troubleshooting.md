---
title: "Solución de problemas"
description: "Soluciones a problemas habituales de Ferrocast: Accesibilidad tras actualizar, el comando brew corto, Gatekeeper, el DMG, ffmpeg ausente y el log."
sidebar:
  order: 3
---
## Se vuelve a pedir Accesibilidad tras `brew upgrade`

La firma ad-hoc se ancla al identificador `dev.ferrocast.launcher`, de modo que macOS debería conservar el permiso entre versiones. Si macOS lo pide de nuevo, concédelo otra vez. Recompilar con firma ad-hoc también puede hacer que macOS lo pida de nuevo.

## `brew install drumst0ck/ferrocast/ferrocast` falla

Añade primero el tap: `brew tap drumst0ck/ferrocast` y después `brew install drumst0ck/ferrocast/ferrocast`. Las versiones recientes de Homebrew rechazan un `brew install ferrocast` a secas desde un tap que no has marcado como de confianza; con el nombre completo se marca solo. Consulta [Instalar](/es/docs/getting-started/install/).

## Gatekeeper bloquea el DMG

El DMG de `scripts/dmg.sh` no está notarizado, así que Gatekeeper lo bloquea en otros Macs. Sirve solo para uso propio. Instala con Homebrew, que compila en tu máquina y la app no lleva marca de cuarentena.

## «ffmpeg is not installed» (o LibreOffice, webp, Ghostscript, unrar)

Falta una herramienta opcional. El mensaje incluye la orden de instalación, por ejemplo `brew install ffmpeg`. Consulta [Herramientas opcionales](/es/docs/files/optional-tools/).

## El hotkey, los snippets o los comandos de ventana no hacen nada

Necesitan Accesibilidad (los snippets pueden necesitar también Input Monitoring). Abre la pantalla Welcome & Permissions o Ajustes del Sistema y concédela. Consulta [Primer arranque](/es/docs/getting-started/first-run/).

## Convert Finder Selection muestra un aviso o no hace nada

La lectura de Finder necesita permiso de Automatización para Finder; mientras el aviso de permiso está pendiente, la llamada vence a los 15 segundos y muestra un aviso en lugar de quedarse colgada.

## Intro sobre un archivo en Finder lo renombra

Es el comportamiento propio de Finder, salvo que actives Ajustes > General > «Enter on Finder selection» (desactivado por defecto, necesita Accesibilidad), que abre la rueda de conversión en lugar de renombrar. También puedes usar Convert Finder Selection o <kbd>⌥</kbd> <kbd>↵</kbd> sobre un resultado de archivo.

## Algo falló sin avisar

Ejecuta el comando **Reveal Log File** en Ferrocast para abrir el log de errores y lee las últimas líneas.

## Relacionado

- [Preguntas frecuentes](/es/docs/reference/faq/)
- [Datos y privacidad](/es/docs/reference/data-and-privacy/)
