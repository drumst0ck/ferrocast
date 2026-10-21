---
title: "Instalar con Homebrew"
description: "Instala Ferrocast en Apple Silicon con Homebrew. La fórmula compila la app en tu Mac, así que no hay cuarentena ni hace falta cuenta de desarrollador."
sidebar:
  order: 1
---
Ferrocast se instala con una fórmula de Homebrew que compila la app desde el código fuente en tu propio Mac. Homebrew instala Rust si no lo tienes. La primera instalación tarda unos minutos.

Ejecuta esto en Terminal:

```bash
brew tap drumst0ck/ferrocast https://github.com/drumst0ck/ferrocast
brew install ferrocast                      # compila en tu Mac (necesita Rust, lo instala brew)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # concede Accesibilidad cuando lo pida
brew upgrade ferrocast                      # para actualizar
```

Usa siempre el `brew tap` con la URL: la forma corta `brew install drumst0ck/ferrocast/ferrocast` intenta clonar `homebrew-ferrocast`, que no existe, y falla.

## Por qué una fórmula y no un cask

El cask oficial de Homebrew exige apps firmadas y notarizadas (Apple Developer, de pago) y desactiva las que no pasan Gatekeeper desde septiembre de 2026. Al compilar desde el código fuente en tu propia máquina, la app **no lleva marca de cuarentena** y Gatekeeper no interviene: no hace falta cuenta de pago ni saltarse ninguna protección. A cambio, la primera instalación tarda unos minutos (compila) y la firma es ad-hoc, de uso local.

## Actualizar

`brew upgrade ferrocast`. La firma ad-hoc se ancla al identificador `dev.ferrocast.launcher` en lugar de al hash del binario, de modo que macOS debería conservar el permiso de Accesibilidad entre versiones. Si tras `brew upgrade` te lo vuelve a pedir, concédelo de nuevo. Consulta [Solución de problemas](/es/docs/reference/troubleshooting/).

## Desinstalar

`brew uninstall ferrocast` elimina la fórmula; borra el enlace `/Applications/Ferrocast.app` creado arriba. Tus datos están en `~/Library/Application Support/Ferrocast`; bórralos tú si quieres eliminarlos.

:::caution[Límites]
Hace falta Apple Silicon; los Mac Intel no son compatibles. No hay descarga notarizada y el DMG no es una descarga pública.
:::

## Relacionado

- [Primer arranque](/es/docs/getting-started/first-run/)
- [Compilar desde el código](/es/docs/getting-started/build-from-source/)
- [Solución de problemas](/es/docs/reference/troubleshooting/)
