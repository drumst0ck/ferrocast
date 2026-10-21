---
title: "Migrar desde Raycast"
description: "Qué se conserva al pasar de Raycast a Ferrocast. Los Script Commands funcionan tal cual, los snippets se importan desde JSON y qué aún no está incluido."
sidebar:
  order: 4
---
Ferrocast está inspirado en Raycast, pero es un proyecto independiente: no está afiliado a Raycast y no usa su marca ni sus assets. Esta es la única página de la documentación que compara ambos.

## Qué se conserva

- Los **Script Commands** funcionan tal cual. Ferrocast lee los metadatos compatibles con Raycast y admite los modos full, compact, silent e inline.
- Los **snippets** se importan desde una exportación JSON de Raycast (ajustes, copia de seguridad e importación).
- Los **quicklinks** los recreas a mano; lleva un minuto.

## Qué no está incluido

Extensiones React, sincronización, Search Menu Bar Items y contexto de IA (selección o pantalla). No hay tienda de extensiones.

## Lado a lado

| | Ferrocast | Raycast (según la página de inicio) |
|---|---|---|
| Precio | Gratis. Licencia MIT. | Nivel gratuito; suscripción Pro para IA y sincronización en la nube. |
| Cuenta | Ninguna. | Opcional; necesaria para las funciones Pro. |
| Extensiones | Script Commands (compatibles con Raycast). Sin tienda. | Miles en la Store. |
| Código fuente | Abierto. | Cerrado. |
| Instalación | Homebrew lo compila desde fuente en tu Mac. | Descarga firmada y notarizada. |

«Raycast» es el nombre de un producto de terceros y se usa aquí solo para comparar.

:::caution[Límites]
Si vives dentro de las extensiones de Raycast, Ferrocast todavía no las reemplaza. Los argumentos de los Script Commands se pasan por línea de comandos, como en Raycast, y pueden verse con `ps` mientras el script corre.
:::

## Relacionado

- [Script Commands](/es/docs/launcher/script-commands/)
- [Preguntas frecuentes](/es/docs/reference/faq/)
