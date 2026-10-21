---
title: "Optional tools"
description: "ffmpeg, LibreOffice, cwebp, Ghostscript and unrar are optional helpers for Ferrocast file tools. See what each unlocks and the Homebrew install line shown."
sidebar:
  order: 8
---
Conversions run with tools that ship with macOS first (sips, afconvert, avconvert, textutil, ditto, tar). Five optional tools unlock more. When one is missing, Ferrocast shows its install command instead of failing silently; on the Convert wheel those outputs are dimmed with the command.

| Tool | Unlocks | Install line shown by the app |
|---|---|---|
| ffmpeg | Audio and video tools, compress for audio and video, and the audio and video conversions that need it. | `brew install ffmpeg` |
| LibreOffice | Document, spreadsheet and presentation conversions, and slides to images. | `brew install --cask libreoffice` |
| cwebp | Writing WebP. | `brew install webp` |
| Ghostscript | Compressing PDF (presets only). | `brew install ghostscript` |
| unrar | RAR extraction fallback. | `brew install --cask rar` |

See the exact pairs in [Conversions](/docs/files/conversions/), where each pair that needs a tool is marked.

:::caution[Limits]
PDF compression requires Ghostscript; there is no native route. The system `tar` reads RAR first and `unrar` is the fallback.
:::

## Related

- [Conversions](/docs/files/conversions/)
- [Compress](/docs/files/tools/compress/)
- [Extract archive](/docs/files/tools/extract-archive/)
