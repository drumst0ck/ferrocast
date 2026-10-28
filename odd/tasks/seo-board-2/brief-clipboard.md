Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb81e989baf8bfc51ab152 as of 2026-10-28T12:07:10.586Z:
<page url="https://app.notion.com/p/4073a474adbb81e989baf8bfc51ab152" icon="📝">
<ancestor-path>
<parent-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"title":"Brief · Clipboard manager for Mac (EN) · /clipboard-manager-mac/"}
</properties>
<iconMetadata>{"type":"emoji","emoji":"📝"}</iconMetadata>
<content>
<b>Fecha:</b> 28-oct-2026 · <b>Idea del tablero:</b> #23 (prioridad 2) · <b>Estado:</b> brief hecho · <b>Tipo:</b> página comercial/alternativa en inglés (frente a Maccy, Paste y Spotlight en Tahoe); hermana EN de <code>/es/historial-portapapeles-mac/</code>

<quote>
Esto es un brief para quien redacte la página, no el texto final. La página es en inglés: términos, title y meta en inglés; el resto del brief en español. Sin volúmenes (no hay herramienta de keywords; Search Console 21–26 oct no muestra consultas de «clipboard»). SERP según nuestra búsqueda del 28-oct-2026, sin posiciones. Lo marcado con ⚠️ no se publica sin comprobarlo.

<b>Dos funciones aún no publicadas</b> (fusionadas, no en v0.3.0): <b>Clear History…</b> y <b>Sync with iCloud</b>. En la página: «coming in the next version». Nunca como disponibles hoy. Ojo: la doc del portapapeles ya las describe como si existieran (ver §8).
</quote>

## 0. Objetivo en una línea

Ser la respuesta en inglés a «free clipboard manager for Mac» para quien quiere un historial que <b>no guarde lo que copia desde su gestor de contraseñas</b>, con una comparación honesta frente a Maccy, Paste y el portapapeles de Spotlight.

## 1. Objetivo y lector

<b>Objetivo:</b> captar «clipboard manager mac», «maccy alternative», «paste app alternative», y convertir en descarga. AEO: respuesta citable a «does Spotlight's clipboard history store passwords?» y «is there a free clipboard manager for Mac?».

<b>Lector y momento:</b>

- Usuario de Mac (a menudo dev) que acaba de descubrir el historial de Spotlight en macOS Tahoe, o que lleva años con Maccy o Paste, y se pregunta qué pasa con las contraseñas que copia.
- Usuario de Paste al que le salta la renovación de la suscripción.
- Momento: comparando; ya sabe lo que es un clipboard manager.

<b>Quién NO es el lector:</b> quien necesita el historial en iPhone/iPad (Paste), o sincronizado entre Macs <b>hoy</b> (en Ferrocast llega en la próxima versión), o un Mac Intel.

## 2. Intención y preguntas reales

<b>Intención:</b> comercial-investigación.

<b>Términos (EN)</b>

- Primario: <code>clipboard manager mac</code>
- Apoyo: <code>free clipboard manager mac</code>, <code>clipboard history mac</code>, <code>maccy alternative</code>, <code>paste app alternative</code>, <code>clipboard manager that ignores passwords</code>, <code>macos tahoe clipboard history</code>, <code>spotlight clipboard history</code>.
- ⚠️ Sin volúmenes.

<b>Preguntas</b>

1. Is there a free clipboard manager for Mac?
2. Does it save passwords I copy from 1Password? How does it know?
3. How is it different from Spotlight's clipboard in macOS Tahoe?
4. Maccy vs Paste vs Ferrocast: which one?
5. How long does it keep history? Can I pin items?
6. Where is the history stored? Is it encrypted?
7. Can I clear everything at once? (Coming in the next version; today: delete entries one by one with ⌃X or set a short retention.)
8. Does it sync between Macs? (Coming in the next version.)

## 3. Qué devuelve hoy la búsqueda y qué le falta

- [https://maccy.app/](https://maccy.app/) (consultado 28-oct): ligero, teclado, MIT, gratis desde la web; «If your password manager removes a copied password from the clipboard, so will Maccy». App Store: 9,99 \$ pago único, macOS 14+, la versión 2.8.0 menciona OCR y pins ([https://apps.apple.com/us/app/maccy/id1527619437](https://apps.apple.com/us/app/maccy/id1527619437)).
- [https://pasteapp.io/pricing](https://pasteapp.io/pricing) (consultado 28-oct): Personal 2,49 $/mes facturado 29,99 $/año; plan Lifetime (precio no visible ⚠️); Mac, iPhone e iPad; pinboards; sync; prueba de 7 días; Setapp.
- [https://support.apple.com/guide/mac-help/search-your-clipboard-history-mchl40d5b86b/mac](https://support.apple.com/guide/mac-help/search-your-clipboard-history-mchl40d5b86b/mac) (consultado 28-oct): ⌘Espacio y luego ⌘4; «Sensitive information may appear on the Clipboard»; Clear History.
- [https://copyagain.app/blog/clipboard-manager-mac-free/](https://copyagain.app/blog/clipboard-manager-mac-free/) (listicle de vendedor), [https://mimer.hasanjafri.com/](https://mimer.hasanjafri.com/) y [https://github.com/momenbasel/pesty](https://github.com/momenbasel/pesty) (OSS).

<b>Respuesta de IA (resumen de nuestro buscador; no es Google AI Overview):</b> lista Maccy, Paste, Raycast; menciona Spotlight en Tahoe. No explica cómo se detectan las contraseñas.

<b>Qué no hace nadie:</b>

1. <b>Nadie explica el mecanismo</b> (los tipos de [nspasteboard.org](http://nspasteboard.org): Concealed, Transient, AutoGenerated) ni qué pasa si una app no marca sus copias.
2. <b>Nadie dice dónde y cómo se guarda</b> el historial (archivo JSON sin cifrar en Ferrocast; dato incómodo pero útil).
3. <b>Pocas comparan con Spotlight de Tahoe</b> usando la guía de Apple.

<b>Honestidad necesaria:</b> saltarse contraseñas <b>no es exclusivo</b>: Maccy también ignora las copias de gestores de contraseñas. El diferencial frente a Maccy no es ese, sino historial + snippets + lanzador + 275 conversiones en una app gratis. Frente a Spotlight, sí es diferencial (Apple avisa de que puede aparecer información sensible).

## 4. URL, title, meta y H1

- <b>URL:</b> <code>https://ferrocast.app/clipboard-manager-mac/</code> · hreflang <code>en</code> ↔ <code>es</code> con <code>/es/historial-portapapeles-mac/</code>; <code>x-default</code> EN.
- <b>Title A:</b> <code>Free Clipboard Manager for Mac That Skips Passwords | Ferrocast</code>
- <b>Title B:</b> <code>Clipboard History for Mac, a Free Maccy and Paste Alternative</code>
- <b>Meta A:</b> <code>A free, open-source clipboard history for Mac that ignores what you copy from password managers. Text, links, colours, files and images, pinning. Compared with Maccy, Paste and Spotlight.</code>
- <b>Meta B:</b> <code>Keep what you copy on your Mac, skip 1Password copies, pin what matters. Free and MIT. How it compares with Spotlight's clipboard in macOS Tahoe, Maccy and Paste.</code>
- <b>H1:</b> <code>A free clipboard manager for Mac that skips your passwords</code>

## 5. Esquema H2/H3 con notas

<b>Entradilla (citable):</b> Ferrocast keeps a local history of the text, links, colours, files and images you copy, so you can paste them later from the launcher. Anything copied from a password manager such as 1Password is skipped. Free, MIT, Apple Silicon.

<b>H2: How it knows what is a password</b>

- Adaptar la sección de la página ES: tipos de [nspasteboard.org](http://nspasteboard.org) (ConcealedType, TransientType, AutoGeneratedType) + 1Password y KeeWeb. Si una app no marca la copia, no puede saberlo: borra con ⌃X. Es el bloque más citable.

<b>H2: Ferrocast vs Spotlight (macOS Tahoe) vs Maccy vs Paste</b> (tabla §6)

<b>H2: Get started</b>

- Instalar (DMG/Homebrew) → Settings → Clipboard (Record history, Keep history for) → copiar → pegar desde la vista Clipboard (pegar necesita Accesibilidad).

<b>H2: Pin, delete and keep what matters</b>

- ⌘. fija, ⌃X borra; panel de detalle (tipo, origen, cuándo, veces, longitud). Retención: 1 day, 1 week, 1 month, 3 months, 1 year o unlimited; lo fijado se queda.
- «Clear History… is coming in the next version.» Botones al pasar el ratón: también próxima versión ⚠️ (no enseñarlos en capturas).

<b>H2: Where your history lives</b>

- <code>clipboard.json</code> en Application Support, sin cifrar; las copias de seguridad de Ferrocast lo excluyen; las imágenes borradas se eliminan del disco. Decirlo sin rodeos; enlazar Data and privacy.

<b>H2: Between Macs</b>

- «Sync with iCloud is coming in the next version. It will be opt-in.» Una o dos frases; nada de detalles como si ya estuviera.

<b>H2: When Maccy or Paste is the better pick</b>

- Maccy: lo más mínimo, también ignora contraseñas, OCR (App Store notes), Intel ⚠️ (comprobar compatibilidad Intel en GitHub).
- Paste: iPhone/iPad, pinboards, sync hoy.

<b>H2: FAQ</b> (§7)

## 6. Tabla comparativa (fuentes 28-oct-2026)

|  | Ferrocast | Spotlight (macOS Tahoe) | Maccy | Paste |
| --- | --- | --- | --- | --- |
| Price | Free (MIT) | Built into macOS | Free (MIT) from site/GitHub; \$9.99 on the App Store | \$2.49/mo billed \$29.99/yr; Lifetime plan ⚠️ price |
| Password-manager copies | Skipped (nspasteboard types + 1Password, KeeWeb) | Apple: «Sensitive information may appear» | «If your password manager removes a copied password… so will Maccy» | ⚠️ not checked |
| Pinning | Yes (⌘.) | Not in Apple's guide | Yes | Pinboards |
| Retention | 1 day → unlimited; pinned stay | ⚠️ 30 min / 8 h / 7 days per MacRumors (26.1) — confirm on Apple | ⚠️ | ⚠️ |
| Clear all | Coming in the next version | Clear History | ⚠️ | ⚠️ |
| Sync | Coming in the next version | — | ⚠️ | Mac, iPhone, iPad |
| Requirements | Apple Silicon, macOS 13+ | macOS Tahoe | macOS 14+ (App Store) | ⚠️ |

Fuentes: §3 + [https://ferrocast.app/docs/launcher/clipboard/](https://ferrocast.app/docs/launcher/clipboard/) + [https://ferrocast.app/es/historial-portapapeles-mac/](https://ferrocast.app/es/historial-portapapeles-mac/)

## 7. Bloque citable (AEO) + FAQ + schema

- <b>Does Spotlight's clipboard history in macOS Tahoe store passwords?</b> Apple's own guide warns that "sensitive information may appear on the Clipboard" and offers Clear History to erase it. Ferrocast, a free Mac clipboard manager, ignores copies that password managers mark as concealed or transient, plus copies from 1Password and KeeWeb.
- <b>Is there a free clipboard manager for Mac?</b> Yes. Ferrocast (MIT) keeps text, links, colours, files and images you copy, lets you pin entries and choose how long history is kept, from 1 day to unlimited. Maccy is another free, open-source option; Paste is a paid subscription with iPhone and iPad apps.
- <b>Is Ferrocast's clipboard history encrypted?</b> No. It is stored in a JSON file on your Mac, unencrypted, and Ferrocast's backup export leaves it out. It is not uploaded anywhere; iCloud sync, opt-in, is coming in the next version.

<b>FAQ visible:</b> las 3 + pinning · retention · clear all («coming in the next version») · Intel.<br /><b>JSON-LD:</b> <code>SoftwareApplication</code> (price 0, macOS 13+) + <code>FAQPage</code> idéntico al visible. Actualizar FAQ y schema cuando salga la versión con Clear History y sync.

## 8. Hechos verificados y caveats

<b>Verificados (28-oct-2026):</b> /docs/launcher/clipboard/ (texto, archivos, imágenes, enlaces y colores; ⌘. y ⌃X; panel de información; omite 1Password; JSON sin cifrar en Application Support; el backup lo excluye; pegar necesita Accesibilidad) · /es/historial-portapapeles-mac/ (retención, nspasteboard, KeeWeb, comparación con Spotlight) · /facts/ · Changelog: Clear History…, botones Pin/Delete al pasar el ratón y Sync with iCloud en <b>Unreleased</b>.

<b>⚠️ Pendiente</b>

- ⚠️ <b>Sitio:</b> /docs/launcher/clipboard/ ya describe Clear History…, los botones al pasar el ratón y Sync with iCloud como disponibles; /docs/launcher/sync/ está publicada. Hasta que salga la versión, añadir «coming in the next version» en la doc o despublicar, para que no contradiga a /facts/ ni a esta página.
- ⚠️ <b>Sitio:</b> /es/historial-portapapeles-mac/ dice «No sale del Mac». Cuando salga sync, matizar: «salvo que actives la sincronización con iCloud».
- ⚠️ Retención de Spotlight (30 min / 8 h / 7 días): solo en la síntesis de búsqueda y MacRumors; confirmar en [support.apple.com](http://support.apple.com).
- ⚠️ Precio del plan Lifetime de Paste; comportamiento de Paste con contraseñas.
- ⚠️ Retención y botón de borrar todo en Maccy; compatibilidad Intel de Maccy.
- ⚠️ Excluir apps a elección del usuario: no documentado en Ferrocast → no afirmar.
- ⚠️ La fila #23 del tablero/CSV menciona sync como si existiera; corregirla.

## 9. Enlaces internos

<b>Salientes:</b> [https://ferrocast.app/docs/launcher/clipboard/](https://ferrocast.app/docs/launcher/clipboard/) · [https://ferrocast.app/docs/reference/data-and-privacy/](https://ferrocast.app/docs/reference/data-and-privacy/) · [https://ferrocast.app/text-expander-mac/](https://ferrocast.app/text-expander-mac/) (cuando exista) · [https://ferrocast.app/raycast-alternative/](https://ferrocast.app/raycast-alternative/) · [https://ferrocast.app/alfred-alternative/](https://ferrocast.app/alfred-alternative/) · [https://ferrocast.app/facts/](https://ferrocast.app/facts/)<br /><b>Entrantes:</b> /es/historial-portapapeles-mac/ (selector de idioma + hreflang) · /raycast-alternative/ y /alfred-alternative/ (sección portapapeles) · /docs/launcher/clipboard/ · /text-expander-mac/ · home EN.

## 10. Pruebas necesarias

1. Captura de la vista Clipboard con texto, imagen, color y enlace, y el panel de información.
2. Prueba: copiar una contraseña desde 1Password → no aparece en el historial (captura antes/después).
3. Captura de Settings → Clipboard (Record history, Keep history for).
4. Captura de Spotlight ⌘Espacio → ⌘4 con el aviso de Apple (o enlace a la guía).
5. Finder: ruta de <code>clipboard.json</code> (sin mostrar contenido sensible).

- No incluir capturas de Clear History… ni de los botones al pasar el ratón hasta que salga la versión.

## 11. Extensión, tono y lo que la hace la mejor respuesta

- <b>Extensión:</b> 1.100–1.500 palabras.
- <b>Tono:</b> inglés claro, sin superlativos; el desarrollador en primera persona («I build Ferrocast»). «Ferrocast is independent and not affiliated with Apple, Maccy or Paste.»
- <b>Lo que la hace la mejor respuesta:</b> es la única que explica cómo se detectan las contraseñas, dónde se guarda el historial (sin cifrar, dicho claro) y compara con la guía de Apple sobre Spotlight, reconociendo que Maccy también se salta contraseñas.
</content>
</page>