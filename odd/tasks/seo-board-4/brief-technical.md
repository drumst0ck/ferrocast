# Brief · Arreglos técnicos: redirección 301 a HTTPS, títulos largos y llms.txt
Source: https://app.notion.com/p/4083a474adbb8167807af1ab6a84b1c9 (copied verbatim 2026-10-29)

<b>Fecha:</b> 29-oct-2026 · <b>Tipo:</b> mejora técnica · <b>Un PR</b> (más un ajuste de servidor si hace falta)

> Brief para quien implemente el cambio. No cambiar URLs, canonical ni hreflang.

## 0. Problemas (dato, revisión 29-oct-2026)

1. <code>curl -sI http://ferrocast.app/</code> → <b>302</b> a <code>https://ferrocast.app/</code>. Debería ser 301 (permanente). <code>www</code> → raíz ya es 301; la barra final ya redirige con 301.
2. Títulos de más de \~60 caracteres que Google cortará: <code>/alfred-alternative/</code> (73), <code>/es/alternativa-a-raycast/</code> (68), <code>/ferrocast-vs-raycast/</code> (68), <code>/es/historial-portapapeles-mac/</code> (68), <code>/convert/md-to-pages/</code> (68), <code>/es/convertir/heic-a-jpg/</code> (65).
3. <code>https://ferrocast.app/llms.txt</code> lista las guías pero <b>no</b> las páginas <code>/tools/*</code> (p. ej. <code>/tools/split-pdf/</code> no aparece). Hay 27 páginas <code>/tools/*</code> en el sitemap.

## 1. Redirección HTTP → HTTPS en 301

- La web la sirve Coolify (proxy Traefik/Caddy). Buscar dónde se fuerza HTTPS: redirección del proxy en la app de Coolify, o regla en Cloudflare si el dominio pasa por Cloudflare. Cambiarla a permanente (301 o 308).
- Si el cambio es en el panel de Coolify o Cloudflare y no en el repo, hacerlo solo si tienes acceso; si no, describirlo en el PR para que lo haga Jose. No tocar otros ajustes del servidor.
- Comprobar: <code>curl -sI http://ferrocast.app/raycast-alternative/</code> → 301 y <code>Location: https://ferrocast.app/raycast-alternative/</code>.

## 2. Títulos ≤ 60 caracteres

Propuestas (palabra clave al principio; total ≤ 60 con el sufijo que use el sitio):

- <code>/alfred-alternative/</code> → <code>Free Alfred Alternative for Mac, No Powerpack · Ferrocast</code>
- <code>/es/alternativa-a-raycast/</code> → <code>Alternativa gratis a Raycast para Mac · Ferrocast</code>
- <code>/ferrocast-vs-raycast/</code> → <code>Ferrocast vs Raycast: Free vs Pro, Compared</code>
- <code>/es/historial-portapapeles-mac/</code> → <code>Historial del portapapeles gratis en Mac · Ferrocast</code>
- <code>/convert/md-to-pages/</code> → <code>Convert Markdown to Pages on Mac, Free · Ferrocast</code>
- <code>/es/convertir/heic-a-jpg/</code> → <code>Convertir HEIC a JPG en Mac gratis · Ferrocast</code>

Notas:

- El separador «·» es orientativo: usar el mismo sufijo que el resto del sitio.
- Antes de cambiar, leer el title actual: si la propuesta pierde el término primario que el actual sí tiene, conservarlo y recortar otra parte.
- Contar caracteres en el HTML final. No cambiar H1 ni meta.
- Añadir al build o a los tests un aviso (warning, no error) si un title de página no-docs pasa de 60 caracteres.

## 3. llms.txt

- En <code>## Primary pages</code> (o una sección nueva <code>## File tools</code>), añadir cada página <code>/tools/*</code> publicada con una línea de descripción sacada de su meta description actual.
- Mantener el formato del archivo; si se genera en build, cambiar el generador, no el archivo de salida.

## 4. Criterios de aceptación

1. HTTP → HTTPS responde 301/308 (o el PR explica qué ajuste debe hacer Jose y dónde).
2. Los 6 titles miden ≤ 60 caracteres en producción.
3. <code>curl -s https://ferrocast.app/llms.txt | grep -c '/tools/'</code> ≥ 20.
4. Ningún otro title, URL o canonical cambia.
