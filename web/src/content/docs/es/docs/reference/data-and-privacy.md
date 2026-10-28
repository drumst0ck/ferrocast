---
title: "Datos y privacidad"
description: "Dónde guarda Ferrocast sus datos, qué se guarda sin cifrar, qué va al Llavero y qué sale de tu Mac: nada, salvo que actives la sincronización con iCloud."
sidebar:
  order: 1
---
Ferrocast guarda sus datos en archivos JSON en `~/Library/Application Support/Ferrocast`. Los archivos no están cifrados y se abren en cualquier editor. Define `FERROCAST_DATA_DIR` para usar otra carpeta (consulta [Variables de entorno](/es/docs/reference/environment-variables/)).

## Qué vive dónde

- **Archivos JSON** en la carpeta de datos: ajustes, historial del portapapeles, snippets, quicklinks, notas y la lista de File Shelf. Las notas están en `notes.json`; una nota eliminada permanece ahí 30 días (Recently Deleted) antes de borrarse definitivamente.
- **Keychain**: las claves de IA (una clave de Anthropic, la clave de un servidor compatible con OpenAI) y la sesión de Sign in with ChatGPT, que se renueva automáticamente.
- **Log**: un registro de errores en la carpeta de datos; ábrelo con el comando **Reveal Log File**.
- **Copias de seguridad**: la exportación deja fuera el historial del portapapeles.

## Qué sale de tu Mac

- Las conversiones y herramientas de archivo nunca suben nada; se ejecutan en local, primero con herramientas de macOS y, si están instalados, ffmpeg, LibreOffice, cwebp, Ghostscript y unrar.
- El historial del portapapeles se queda en tu Mac salvo que actives [Sincronizar con iCloud](/es/docs/launcher/sync/) (desactivado por defecto); entonces se copia a tu iCloud Drive para llegar a tus otros Macs. Lo copiado desde un gestor de contraseñas como 1Password nunca se guarda, así que tampoco se sincroniza.
- Las tasas de cambio vienen de frankfurter.dev (datos del BCE, en caché 12 horas).
- Ask AI habla por defecto con Ollama en tu propio Mac. Solo contacta con la nube si lo apuntas a LM Studio, OpenAI, la API de Anthropic, Sign in with ChatGPT o Claude Code.
- Los comandos de IA de Notes (Fix Spelling and Grammar, Summarize y los demás) envían el texto seleccionado, o la nota entera, al proveedor configurado en Ajustes → AI. Con Ollama se queda en tu Mac.
- El dictado de Notes usa el micrófono solo mientras dictas. El audio va al reconocimiento de voz de Apple, que se ejecuta en tu Mac cuando el sistema lo admite, y solo mientras el dictado está activo.
- Las URL `ferrocast://` (por ejemplo `ferrocast://notes/new`) solo navegan: abren una nota, crean una o muestran u ocultan la ventana de notas.
- Los argumentos de los Script Commands se pasan por la línea de comandos y se ven con `ps` mientras el script se ejecuta.

:::caution[Límites]
Los archivos JSON no están cifrados. Las imágenes eliminadas del portapapeles se borran del disco, y los guardados fallidos ahora se notifican.
:::

## Relacionado

- [Historial del portapapeles](/es/docs/launcher/clipboard/)
- [Ask AI](/es/docs/launcher/ask-ai/)
- [Notas](/es/docs/launcher/notes/)
- [Ajustes](/es/docs/launcher/settings/)
