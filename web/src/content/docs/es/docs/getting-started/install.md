---
title: "Instalar con Homebrew"
description: "Instala Ferrocast en Apple Silicon con Homebrew. La fórmula compila la app en tu Mac, así que no hay cuarentena ni hace falta cuenta de desarrollador."
sidebar:
  order: 1
---
Ferrocast se instala con una fórmula de Homebrew que compila la app desde el código fuente en tu propio Mac. Homebrew instala Rust si no lo tienes. La primera instalación tarda unos minutos.

Ejecuta esto en Terminal:

```bash
brew tap drumst0ck/ferrocast
brew install drumst0ck/ferrocast/ferrocast  # compila en tu Mac (necesita Rust, lo instala brew)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # concede Accesibilidad cuando lo pida
brew upgrade ferrocast                      # para actualizar
```

Ejecuta antes `brew tap`: Homebrew se niega a cargar fórmulas de un tap que no has añadido. La fórmula vive en [drumst0ck/homebrew-ferrocast](https://github.com/drumst0ck/homebrew-ferrocast) y sigue cada versión estable de forma automática.

## Descargar el DMG

[Descarga el último DMG](https://github.com/drumst0ck/ferrocast/releases/latest/download/Ferrocast.dmg). Cada versión estable se publica también en la [página de Releases de GitHub](https://github.com/drumst0ck/ferrocast/releases) como `Ferrocast-X.Y.Z.dmg`, con un `SHA256SUMS.txt`. Abre el DMG, arrastra Ferrocast a Aplicaciones y ábrela. La app lleva firma ad-hoc y no está notarizada, así que macOS bloquea el primer arranque de una descarga del navegador: abre Ajustes del Sistema → Privacidad y seguridad y elige «Abrir igualmente». Solo Apple Silicon.

## Actualizaciones

- **Instalación por descarga**: Ferrocast consulta GitHub una vez al día por una versión estable nueva (las prereleases se ignoran). Por defecto muestra las notas de la versión y te deja instalar, aplazar u omitir esa versión; puedes pasar a instalación automática, desactivar la comprobación diaria o pulsar «Check now» en Ajustes → General → Updates. El menú de la barra y Acerca de también tienen «Check for Updates…».
- **Instalación con Homebrew**: la app nunca se reemplaza a sí misma; las actualizaciones ejecutan `brew upgrade ferrocast`.
- Una autoactualización reemplaza la app por la nueva. Como la firma es ad-hoc, macOS puede pedir Accesibilidad otra vez; concédela de nuevo.

## Por qué una fórmula y no un cask

El cask oficial de Homebrew exige apps firmadas y notarizadas (Apple Developer, de pago) y desactiva las que no pasan Gatekeeper desde septiembre de 2026. Al compilar desde el código fuente en tu propia máquina, la app **no lleva marca de cuarentena** y Gatekeeper no interviene: no hace falta cuenta de pago ni saltarse ninguna protección. A cambio, la primera instalación tarda unos minutos (compila) y la firma es ad-hoc, de uso local.

## Actualizar

`brew upgrade ferrocast`. La firma ad-hoc se ancla al identificador `dev.ferrocast.launcher` en lugar de al hash del binario, de modo que macOS debería conservar el permiso de Accesibilidad entre versiones. Si tras `brew upgrade` te lo vuelve a pedir, concédelo de nuevo. Consulta [Solución de problemas](/es/docs/reference/troubleshooting/).

## Desinstalar

`brew uninstall ferrocast` elimina la fórmula; borra el enlace `/Applications/Ferrocast.app` creado arriba. Tus datos están en `~/Library/Application Support/Ferrocast`; bórralos tú si quieres eliminarlos.

:::caution[Límites]
Hace falta Apple Silicon; los Mac Intel no son compatibles. El DMG no está notarizado.
:::

## Relacionado

- [Primer arranque](/es/docs/getting-started/first-run/)
- [Compilar desde el código](/es/docs/getting-started/build-from-source/)
- [Solución de problemas](/es/docs/reference/troubleshooting/)
