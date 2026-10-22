---
title: "La rueda al arrastrar"
description: "Arrastra archivos y pulsa Shift para conversiones, Command para herramientas o Control para acciones. La rueda de Ferrocast queda abierta hasta soltar."
sidebar:
  order: 2
---
La rueda al arrastrar es un anillo de opciones que aparece bajo el puntero mientras arrastras archivos. Hay tres ruedas, cada una con su acento: **Convert** (naranja), **Tools** (azul) y **Actions** (verde). Para abrirla no hace falta el permiso de Accesibilidad.

Empieza a arrastrar archivos y pulsa <kbd>⇧</kbd> para conversiones, <kbd>⌘</kbd> para herramientas o <kbd>⌃</kbd> para acciones. Suelta sobre un segmento para ejecutarlo; la rueda sigue abierta hasta que sueltas o cancelas. Pulsar otra tecla cambia la rueda abierta. También puedes abrirla desde un resultado de archivo con <kbd>⌥</kbd> <kbd>↵</kbd>; las flechas eligen, <kbd>↵</kbd> aplica y <kbd>esc</kbd> cancela. Las teclas y el interruptor de activado están en Ajustes → General.

## Diseño

Un anillo «halo»: un acento por rueda, sector resaltado con degradado y brillo, centro con el nombre de la rueda, el recuento y el tipo, y todos los segmentos con su nombre bajo el icono (desde 9 la letra es más pequeña y los nombres largos se cortan con «…»; el nombre completo sale en el centro al pasar por encima). La rueda de acciones tiene las seis acciones de archivo más Zip; la de conversión muestra todas las salidas, atenuando las que necesitan una herramienta no instalada y mostrando la orden de instalación.

## Traducir texto arrastrado

Arrastra texto seleccionado (desde cualquier app que permita arrastrar una selección) y toca la tecla de traducción (<kbd>⌘</kbd> por defecto; <kbd>⇧</kbd> ampliaría la selección, por eso no se usa). Se abre una cuarta rueda, morada, con tus idiomas; suelta sobre uno para traducir. Por defecto una ventana de Ferrocast muestra la traducción (también se copia al portapapeles) con los botones Copy y «Paste into app», así que sirve en páginas que solo lees. Ajustes > General > «After translating» permite pegarla sobre la selección o solo copiarla.

La traducción usa los mismos proveedores de IA que Ask AI: un servidor compatible con OpenAI (Ollama lo mantiene en tu Mac), Anthropic, Sign in with ChatGPT o Claude Code (tus propias suscripciones). Elige el motor en Ajustes > AI > Translation, o déjalo en «Same as chat». El texto seleccionado se envía a ese proveedor y a nadie más. Los idiomas son una lista separada por comas en Ajustes > General (hasta 12). Las selecciones de más de 20.000 caracteres se rechazan.

Esta rueda no tiene otra a la que cambiar, así que <kbd>⌘</kbd> y <kbd>⌃</kbd> no hacen nada mientras está abierta.

## Herramientas según el formato

Una tabla de 26 herramientas decide qué ofrecen la rueda de herramientas y la vista Convertir para cada tipo de archivo; las que no tienen ventana usan presets o comandos escritos.

## Convert Finder Selection

Un comando (asígnale un hotkey en los ajustes) que lee lo seleccionado en Finder y abre la rueda de conversión para ello. No intercepta Intro, así que el cambio de nombre de Finder sigue funcionando salvo que actives el ajuste de abajo. La lectura de Finder necesita permiso de Automatización.

## Intro sobre la selección de Finder

Un ajuste opcional en Ajustes > General, fila «Enter on Finder selection»: pulsa Intro sobre archivos seleccionados en Finder para abrir la rueda de conversión en lugar de renombrar. Necesita acceso de Accesibilidad y está desactivado por defecto. El comando Convert Finder Selection también funciona.

## Arreglo al soltar

Los archivos arrastrados desde Finder llegan como URL de referencia de archivo (`file:///.file/id=…`); ahora se resuelven a la ruta real. Antes, un PNG arrastrado parecía un archivo sin extensión.

:::caution[Límites]
Todavía no hay animaciones. Intro sobre un resultado de archivo sigue abriéndolo.
:::

## Relacionado

- [Conversiones](/es/docs/files/conversions/)
- [Acciones de archivo](/es/docs/files/file-actions/)
- [Ajustes](/es/docs/launcher/settings/)
