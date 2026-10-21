---
title: "Preguntas frecuentes"
description: "Respuestas sobre Ferrocast: precio, instalación, permisos, privacidad, Ask AI, planes de ChatGPT y Claude, Raycast, conversión y qué falta hoy."
sidebar:
  order: 4
---
## General

### ¿Cómo lo abro?
Pulsa <kbd>⌥</kbd> <kbd>Space</kbd> desde cualquier app. Puedes cambiar el atajo en los ajustes.

### ¿Es gratis?
Del todo. Licencia MIT, sin cuenta y sin un nivel Pro detrás de un muro de pago.

### ¿Funcionará en mi Mac?
Funciona en Macs con Apple Silicon. Los Mac Intel no son compatibles.

### ¿Necesito ser desarrollador para instalarlo?
No, pero tienes que abrir Terminal una vez y pegar cuatro líneas. Homebrew hace el resto y compila durante unos minutos. Consulta [Instalar](/es/docs/getting-started/install/).

### ¿Por qué pide acceso de Accesibilidad?
Porque mover ventanas, pegar por ti y expandir snippets lo necesitan. La búsqueda, la calculadora y la IA funcionan sin él. Una pantalla de bienvenida te guía.

### ¿Mi historial del portapapeles es privado?
Nunca sale de tu Mac. Está en un archivo JSON en Application Support, y lo que se copia desde un gestor de contraseñas como 1Password se ignora a propósito.

### ¿Qué falta hoy?
Con honestidad: una tienda de extensiones y la sincronización entre Macs. Pulsar Intro sobre una selección de Finder todavía no está incluido (hay un comando para ello). Si vives dentro de las extensiones de Raycast, Ferrocast todavía no las reemplaza.

## IA y migración

### ¿Ask AI envía mi texto a la nube?
Solo si tú lo indicas. De fábrica habla con Ollama en tu propio Mac. Apúntalo a LM Studio, OpenAI o la API de Anthropic si lo prefieres; las claves van al Keychain.

### ¿Puedo usar mi suscripción de ChatGPT o de Claude?
Sí, con matices que la app explica. Claude: si Claude Code está instalado y con sesión iniciada, Ferrocast lo usa como proveedor, porque es la herramienta de Anthropic. ChatGPT: Ferrocast puede iniciar sesión con tu cuenta como hace la CLI Codex de OpenAI; funciona, pero OpenAI no ha bendecido que apps de terceros lo hagan, así que la API con clave es la vía correcta. Consulta [Ask AI](/es/docs/launcher/ask-ai/).

### Vengo de Raycast. ¿Funcionará lo mío?
Tus Script Commands funcionan tal cual y los snippets se importan desde una exportación JSON. Los quicklinks los recreas en un minuto. Consulta [Migrar desde Raycast](/es/docs/getting-started/migrating-from-raycast/).

## Conversión de archivos

### ¿Las conversiones son realmente locales?
Sí. No se sube nada. Las conversiones se ejecutan en tu Mac y usan primero las herramientas que incluye macOS. ffmpeg, LibreOffice, cwebp, unrar y Ghostscript son opcionales y solo se usan si están instaladas; la [referencia de conversiones](/es/docs/files/conversions/) indica qué pares necesitan una.

### ¿Qué puede convertir?
Hay 46 formatos de entrada y 275 pares de conversión en imágenes, audio, vídeo, PDF, documentos, presentaciones, hojas de cálculo, subtítulos y archivos comprimidos.

### ¿Está probada cada conversión?
Cada par se ejecuta en la batería de tests sobre archivos de muestra generados y se lee el resultado.

### ¿Sobrescribe mis archivos?
No. El resultado se escribe junto al original con el mismo nombre y la nueva extensión. Si ese nombre está ocupado añade 2, 3 y así sucesivamente.

### ¿Puedo pulsar Intro sobre un archivo en Finder?
Todavía no. Un comando del lanzador llamado Convert Finder Selection lee la selección de Finder y abre la rueda de formatos, y Option-Intro sobre un resultado de archivo en el lanzador hace lo mismo. Intro a secas sobre una selección de Finder no está incluido.

### ¿Se han usado de verdad las ventanas y la rueda?
Sí. Sus ventanas se renderizan y se comprueban con capturas, con tests automáticos para teclas y arrastres.

## Relacionado

- [Estado de las funciones](/es/docs/reference/status/)
- [Solución de problemas](/es/docs/reference/troubleshooting/)
