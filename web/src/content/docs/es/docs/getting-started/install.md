---
title: "Instalar Ferrocast"
description: "Descarga el DMG de Ferrocast para Apple Silicon o Intel y arrástralo a Aplicaciones, o instálalo con Homebrew como alternativa que compila en tu Mac."
sidebar:
  order: 1
---
## Descargar el DMG

[Descarga el último DMG](https://github.com/drumst0ck/ferrocast/releases/latest/download/Ferrocast.dmg), ábrelo, arrastra Ferrocast a Aplicaciones y ábrela. La app lleva firma ad-hoc y no está notarizada, así que macOS bloquea el primer arranque de una descarga del navegador: abre Ajustes del Sistema → Privacidad y seguridad y elige «Abrir igualmente». Todas las versiones, con un `SHA256SUMS.txt`, están en la [página de Releases de GitHub](https://github.com/drumst0ck/ferrocast/releases). Apple Silicon e Intel (universal; Intel no probado en hardware real).

## Alternativa: instalar con Homebrew

La fórmula de Homebrew compila la app desde el código fuente en tu propio Mac, así que la app no lleva marca de cuarentena y macOS no muestra ningún aviso. Homebrew instala Rust si no lo tienes y la primera instalación tarda unos minutos. Ejecuta esto en Terminal:

```bash
brew tap drumst0ck/ferrocast
brew install drumst0ck/ferrocast/ferrocast  # compila en tu Mac (necesita Rust, lo instala brew)
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" /Applications/Ferrocast.app
open /Applications/Ferrocast.app            # concede Accesibilidad cuando lo pida
brew upgrade ferrocast                      # para actualizar
```

Ejecuta antes `brew tap`: Homebrew se niega a cargar fórmulas de un tap que no has añadido. La fórmula vive en [drumst0ck/homebrew-ferrocast](https://github.com/drumst0ck/homebrew-ferrocast) y sigue cada versión estable de forma automática.

## Actualizaciones

- **Instalación por descarga**: Ferrocast consulta GitHub cada hora más o menos por una versión estable nueva (las prereleases se ignoran), poco después de arrancar y otra vez al abrir el launcher si la última comprobación tiene más de 15 minutos. Si una comprobación falla, se reintenta a los 15 minutos. Cuando hay una versión más nueva, la barra de búsqueda raíz recibe un contorno de acento y, en su extremo derecho, el logotipo de Ferrocast con «Actualizar a {version}»; púlsalo (o pulsa ⌘U) para instalar al momento. Nada aparece por sí solo. Puedes pasar a instalación automática, desactivar la comprobación automática o pulsar «Buscar ahora» en Ajustes → General → Actualizaciones; el menú de la barra y Acerca de también tienen «Buscar actualizaciones…», que abre las notas de la versión para instalar, aplazar u omitir. El indicador no aparece para una versión omitida, en builds de desarrollo ni cuando la carpeta de la app no es escribible.
- **Instalación con Homebrew**: la app nunca se reemplaza a sí misma; el aviso de la barra de búsqueda e Instalar ejecutan `brew upgrade ferrocast`.
- Una autoactualización reemplaza la app por la nueva. Como la firma es ad-hoc, macOS puede pedir Accesibilidad otra vez; concédela de nuevo.

## Por qué una fórmula y no un cask

El cask oficial de Homebrew exige apps firmadas y notarizadas (Apple Developer, de pago) y desactiva las que no pasan Gatekeeper desde septiembre de 2026. Al compilar desde el código fuente en tu propia máquina, la app **no lleva marca de cuarentena** y Gatekeeper no interviene: no hace falta cuenta de pago ni saltarse ninguna protección. A cambio, la primera instalación tarda unos minutos (compila) y la firma es ad-hoc, de uso local.

## Actualizar

`brew upgrade ferrocast`. La firma ad-hoc se ancla al identificador `dev.ferrocast.launcher` en lugar de al hash del binario, de modo que macOS debería conservar el permiso de Accesibilidad entre versiones. Si tras `brew upgrade` te lo vuelve a pedir, concédelo de nuevo. Consulta [Solución de problemas](/es/docs/reference/troubleshooting/).

## Desinstalar

`brew uninstall ferrocast` elimina la fórmula; borra el enlace `/Applications/Ferrocast.app` creado arriba. Tus datos están en `~/Library/Application Support/Ferrocast`; bórralos tú si quieres eliminarlos.

:::caution[Límites]
El DMG/zip funciona en Macs Apple Silicon e Intel (una única descarga universal; Intel no está probado en hardware real). Homebrew compila desde el código fuente para la arquitectura de tu Mac. El DMG no está notarizado.
:::

## Relacionado

- [Primer arranque](/es/docs/getting-started/first-run/)
- [Compilar desde el código](/es/docs/getting-started/build-from-source/)
- [Solución de problemas](/es/docs/reference/troubleshooting/)
