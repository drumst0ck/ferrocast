# Ferrocast: estado de la primera versión

Objetivo: un launcher macOS escrito en Rust, inspirado en Raycast, con identidad
propia. No es una réplica completa ni ejecuta las extensiones React de Raycast.

## Trabajo y verificación

- Núcleo y bindings macOS existentes: commits `771be3d` y `af15540`.
- UI y servicios existentes retomados sin rehacerlos: `crates/app/src`.
- Bloqueo de compilación: egui 0.36 exige Rust 1.95. Se instala y se fija
  solo para este repo con `rust-toolchain.toml`, sin cambiar el toolchain global.
- Estado final y límites: ver README.
  arrancar visible y oculto, empaquetar `.app` y documentar límites reales.

## Decisiones

- Nombre provisional: Ferrocast, para no reutilizar la marca ni assets de Raycast.
- UI Rust con egui, no AppKit nativo: los iconos de apps sí vienen de Finder;
  los emoji son monocromos. Prioridad: versión funcional y testeable.
- Primero macOS/Apple Silicon. AI, sincronización, marketplace React y paridad
  completa se dejan para iteraciones posteriores.
- Datos locales en JSON. No se debe afirmar que el historial está cifrado.
- Pruebas del arranque no ejercitan apagado, reinicio, borrado ni scripts ajenos.

## Pendiente

El estado final de funciones y la evidencia de pruebas se publicarán en README
al terminar la comprobación. Los permisos TCC de macOS deben concederse por el
usuario; ningún script intenta modificarlos ni desactivar las protecciones.
