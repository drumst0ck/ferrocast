# Brief · Imágenes en guías: capturas existentes + og:image por página (EN+ES)
Source: https://app.notion.com/p/4083a474adbb81719f1ddc7a859d7088 (copied verbatim 2026-10-29)

<b>Fecha:</b> 29-oct-2026 · <b>Tipo:</b> mejora de páginas existentes · <b>Un PR</b>

> Brief para quien implemente el cambio en <code>drumst0ck/ferrocast</code> (<code>web/</code>). No es texto final. Solo se usan capturas que ya existen en <code>web/public/img/</code>; no inventar ni editar capturas.

## 0. Problema (dato, revisión 29-oct-2026)

- Las guías y comparativas (<code>/raycast-alternative/</code>, <code>/clipboard-manager-mac/</code>, <code>/tools/split-pdf/</code>, <code>/convert/*</code>, sus versiones <code>/es/</code>…) tienen <b>una sola imagen</b>: el icono <code>/icon.png</code> con <code>alt=""</code>. Ninguna captura del producto.
- <b>Todas</b> las páginas usan la misma <code>og:image</code>: <code>https://ferrocast.app/og-welcome.jpg</code>. Al compartir en X, Reddit o Slack, o en tarjetas de buscadores con IA, todas se ven iguales.
- La portada y <code>/files/</code> ya usan 38 capturas en <code>/img/</code> con <code>alt</code>, <code>width</code>, <code>height</code> y <code>loading="lazy"</code>.

## 1. Capturas en el cuerpo de cada guía

Insertar 1–3 capturas existentes por página, justo después de la entradilla y en la sección de pasos. Mismo marcado que la portada (<code>width</code>, <code>height</code>, <code>loading="lazy"</code>, <code>decoding="async"</code>) y <code>alt</code> descriptivo en el idioma de la página (ES en <code>/es/</code>).

| Página (EN + su ES si existe) | Capturas |
| --- | --- |
| raycast-alternative, ferrocast-vs-raycast, alfred-alternative, raycast-vs-alfred-vs-spotlight | <code>search.jpg</code>, <code>panel.jpg</code>, <code>settings.jpg</code> |
| clipboard-manager-mac / historial-portapapeles-mac | <code>clipboard.jpg</code> |
| text-expander-mac / expansor-de-texto-mac | <code>snippets.jpg</code> |
| local-ai-launcher / ia-local-mac | <code>ai.jpg</code> |
| screen-studio-alternative / grabar-pantalla-mac-zoom-automatico | <code>record-editor.jpg</code> |
| cleanshot-alternative / captura-de-pantalla-con-scroll-mac | <code>files-annotate.jpg</code> |
| blur-video-mac / difuminar-video-mac | <code>files-vredact.jpg</code>, <code>files-redact.jpg</code> |
| compress-video-to-size / comprimir-video-a-un-tamano | <code>files-compress-video.jpg</code> |
| editar-video-mac-sin-imovie | <code>files-vtrim.jpg</code>, <code>files-vsplit.jpg</code>, <code>files-vspeed.jpg</code> |
| remove-metadata-mac / quitar-metadatos-mac | <code>files-meta.jpg</code> |
| free-file-converter-mac / convertidor-de-archivos-gratis-mac | <code>files-wheel-convert.jpg</code>, <code>files-wheel-tools.jpg</code> |
| unir-pdf-mac, tools/organize-pdf | <code>files-organize.jpg</code> |
| tools/crop-image · crop-video · split-video · trim-audio · edit-photos · edit-metadata | <code>files-crop.jpg</code> · <code>files-vcrop.jpg</code> · <code>files-vsplit.jpg</code> · <code>files-atrim.jpg</code> · <code>files-edit.jpg</code> · <code>files-meta.jpg</code> |
| plantilla <code>/convert/*</code> y <code>/es/convertir/*</code> | <code>files-wheel-convert.jpg</code> (una vez, en «How to») |

Si una página no tiene captura adecuada en la tabla (p. ej. <code>mac-window-management</code>, <code>translate-selected-text-mac</code>, <code>raycast-script-commands</code>), <b>no poner ninguna</b> y listarla en el PR para que Jose haga la captura.

Además: dar <code>alt="Ferrocast"</code> al icono de la cabecera (hoy <code>alt=""</code>).

## 2. og:image por página

- Generar en build una imagen 1200×630 por página no-docs con el H1 (o title sin el sufijo «\| Ferrocast») sobre la plantilla de marca de <code>og-welcome.jpg</code>. En Astro vale un endpoint estático con Satori/<code>@vercel/og</code> o similar; elegir lo que menos dependencias añada.
- <code>og:image</code>, <code>og:image:width</code>, <code>og:image:height</code>, <code>og:image:alt</code> y <code>twitter:image</code> apuntan a la imagen propia. <code>twitter:card</code> = <code>summary_large_image</code>.
- Docs (Starlight) pueden quedarse con la imagen genérica en este PR.
- No cambiar title, meta, canonical ni hreflang.

## 3. Criterios de aceptación

1. Cada página de la tabla §1 tiene ≥ 1 <code><img></code> de <code>/img/</code> con <code>alt</code> no vacío, en su idioma.
2. <code>curl -s https://ferrocast.app/clipboard-manager-mac/ | grep og:image</code> devuelve una URL distinta de <code>og-welcome.jpg</code>, y esa URL responde 200 con <code>image/png</code> o <code>image/jpeg</code>.
3. Dos páginas distintas tienen <code>og:image</code> distinta.
4. Lighthouse: sin CLS nuevo (las imágenes llevan <code>width</code>/<code>height</code>).
5. El PR lista las páginas sin captura disponible.

## 4. Después (SEO & AEO Desk)

Revisar en el informe de rendimiento de Search Console si aparecen impresiones en búsqueda de imágenes (tipo <code>image</code>) hacia el 12-nov-2026.
