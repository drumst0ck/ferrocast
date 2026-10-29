# Brief · README de GitHub (EN+ES): enlazar las guías y comparativas
Source: https://app.notion.com/p/4083a474adbb81a29f87cf494e9f1190 (copied verbatim 2026-10-29)

<b>Fecha:</b> 29-oct-2026 · <b>Tipo:</b> cambio en el repo (sin cambios en la web) · <b>Un PR</b>

> Brief para quien implemente el cambio. Solo se tocan <code>README.md</code> y <code>README.es.md</code> del repo <code>drumst0ck/ferrocast</code>. No cambiar ningún dato del README; solo añadir enlaces.

## 0. Problema (dato, 29-oct-2026)

El README enlaza la portada, docs, changelog, <code>/facts/</code>, <code>/llms.txt</code> y <code>/files/</code>, pero <b>ninguna</b> de las guías o comparativas: ni <code>/raycast-alternative/</code>, ni <code>/alfred-alternative/</code>, ni <code>/free-file-converter-mac/</code>. Por ahora [github.com](http://github.com) es prácticamente el único sitio externo con autoridad que enlaza a [ferrocast.app](http://ferrocast.app), y Google descubre y valora mejor las páginas enlazadas desde fuera.

## 1. Cambios en <code>README.md</code>

1. <b>Línea de navegación (línea 11):</b> tras <code>[Changelog](…)</code> añadir <code>· [Raycast alternative](https://ferrocast.app/raycast-alternative/)</code>.
2. <b>Sección </b><b><code>## Raycast and Alfred</code></b><b>:</b> al final, una línea:<br /><code>More: [Ferrocast vs Raycast](https://ferrocast.app/ferrocast-vs-raycast/) · [Raycast alternative](https://ferrocast.app/raycast-alternative/) · [Alfred alternative](https://ferrocast.app/alfred-alternative/) · [Script Commands](https://ferrocast.app/raycast-script-commands/)</code>
3. <b>Sección </b><b><code>## What it does</code></b><b>:</b> donde se nombren las funciones, enlazar cada una a su guía la primera vez que aparece (solo si la palabra ya está en el texto; no reescribir frases):
   - clipboard history → <code>/clipboard-manager-mac/</code>
   - snippets → <code>/text-expander-mac/</code>
   - window management → <code>/mac-window-management/</code>
   - Ask AI / local AI → <code>/local-ai-launcher/</code>
   - screen recording → <code>/screen-studio-alternative/</code>
   - screenshots / capture → <code>/cleanshot-alternative/</code>
   - translate → <code>/translate-selected-text-mac/</code>
4. <b>Sección </b><b><code>## File tools</code></b><b>:</b> junto a «Browse every pair at …/files/», añadir <code>· [Free file converter for Mac](https://ferrocast.app/free-file-converter-mac/) · [Remove metadata](https://ferrocast.app/remove-metadata-mac/) · [Blur part of a video](https://ferrocast.app/blur-video-mac/)</code>.

## 2. Cambios en <code>README.es.md</code>

Lo mismo con las URLs <code>/es/</code>: <code>/es/alternativa-a-raycast/</code>, <code>/es/raycast-vs-alfred-vs-spotlight/</code>, <code>/es/alternativa-a-alfred/</code>, <code>/es/historial-portapapeles-mac/</code>, <code>/es/expansor-de-texto-mac/</code>, <code>/es/gestion-de-ventanas-mac/</code>, <code>/es/ia-local-mac/</code>, <code>/es/grabar-pantalla-mac-zoom-automatico/</code>, <code>/es/captura-de-pantalla-con-scroll-mac/</code>, <code>/es/traducir-texto-seleccionado-mac/</code>, <code>/es/convertidor-de-archivos-gratis-mac/</code>, <code>/es/quitar-metadatos-mac/</code>, <code>/es/difuminar-video-mac/</code>. Si un archivo <code>README.es.md</code> no tiene la sección equivalente, añadir solo la línea de navegación y la línea «Más:».

## 3. Criterios de aceptación

1. Todos los enlaces nuevos devuelven 200 (comprobar con <code>curl -s -o /dev/null -w '%{http_code}'</code>).
2. Ningún texto o dato existente cambia, salvo convertir palabras en enlaces.
3. Sin enlaces EN en el README ES ni al revés (salvo el selector de idioma).
4. Texto de enlace descriptivo, nunca «here» o «aquí».

## 4. Después (SEO & AEO Desk)

Comprobar en Search Console → Enlaces, hacia el 12-nov-2026, si [github.com](http://github.com) aparece como sitio que enlaza y a qué páginas.
