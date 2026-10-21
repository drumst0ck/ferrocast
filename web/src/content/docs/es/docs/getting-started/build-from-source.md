---
title: "Compilar desde el código"
description: "Compila el paquete de la app Ferrocast y un DMG desde el código fuente con los scripts del repositorio. Requiere Rust 1.95 y un Mac con Apple Silicon."
sidebar:
  order: 3
---
Puedes compilar Ferrocast sin Homebrew desde una copia del repositorio. Así lo ejecutan también quienes contribuyen.

```bash
scripts/bundle.sh          # compila y crea dist/Ferrocast.app con icono (firma ad-hoc, solo local)
scripts/dmg.sh             # dist/Ferrocast-<versión>.dmg
open dist/Ferrocast.app    # atajo por defecto: ⌥ Space; la primera vez muestra Welcome & Permissions
cargo test --workspace     # ~70 tests
scripts/shot.sh out.png FERROCAST_QUERY="100 usd in eur"   # renderiza la UI a un PNG (sin permiso de pantalla)
scripts/perf.sh            # tabla de rendimiento base (arranque, reposo, vistas); FERROCAST_PROFILE=1 imprime frames lentos
```

Requiere Rust 1.95 (fijado en `rust-toolchain.toml`; lo exige egui 0.36) y macOS en Apple Silicon. Los datos viven en `~/Library/Application Support/Ferrocast`; `FERROCAST_DATA_DIR` cambia la ruta. Consulta [Variables de entorno](/es/docs/reference/environment-variables/).

## Distribución

La vía soportada es Homebrew compilando desde fuente. El DMG de `scripts/dmg.sh` **sin notarizar** lo bloqueará Gatekeeper en otros Macs que lo descarguen; solo sirve para uso propio. La notarización (`scripts/notarize.sh`) es opcional y requiere cuenta de desarrollador de pago.

:::caution[Límites]
El script de notarización es opcional. Los Mac Intel no son compatibles.
:::

## Relacionado

- [Instalar con Homebrew](/es/docs/getting-started/install/)
- [Contribuir](/es/docs/contributing/)
