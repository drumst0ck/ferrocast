Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb819a96befe8874bcb953 as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb819a96befe8874bcb953">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"Rankean páginas en inglés ([opencapture.app/how-to/scrolling-screenshot-mac.html](http://opencapture.app/how-to/scrolling-screenshot-mac.html)), repos de GitHub (sw33tLie/macshot, codingo/macshot, README de Capso) y una sola página nativa en español de una app pequeña ([shotscreen.app/es/scrolling-screenshot-mac](http://shotscreen.app/es/scrolling-screenshot-mac)). Ferrocast no tiene nada en español sobre capturas salvo /es/docs/launcher/screen-capture. (2026-10-28, búsqueda pública)","Idea":"Tarea /es/captura-de-pantalla-con-scroll-mac: \"Hacer una captura de pantalla con scroll (página completa) en Mac gratis\" y copiar el texto de una captura.","Idioma":"ES","Intención":"Tarea (cómo hacerlo / qué app usar)","Por qué es ganable":"Pregunta muy concreta (macOS no lo hace de serie) con casi nada en español. Ferrocast tiene Capture Scrolling y copia de texto de la captura; página corta y fácil de hacer, que enlaza a /es/grabar-pantalla-mac-zoom-automatico.","Prioridad":2,"Público":"Usuarios hispanohablantes de Mac que necesitan capturar chats, webs o documentos largos","Términos objetivo":"captura de pantalla con scroll mac; captura de página completa mac; captura larga mac; capturar pantalla entera web mac; copiar texto de una captura mac","url":"https://app.notion.com/p/4073a474adbb819a96befe8874bcb953"}
</properties>
<iconMetadata>null</iconMetadata>
<blank-page>This page is blank and has no content.</blank-page>
</page>

<parent-data-source-schema url="collection://6de35012-1676-40c6-a1d7-281ff598db92">
The SQLite table definition for this page's parent data source. Use these column names and option values to set this page's properties, create sibling pages, or query the data source.
<sqlite-table>
CREATE TABLE IF NOT EXISTS "collection://6de35012-1676-40c6-a1d7-281ff598db92" (
	url TEXT UNIQUE,
	createdTime TEXT, -- ISO-8601 datetime string, automatically set. This is the canonical time for when the page was created.
	"Prioridad" FLOAT,
	"Intención" TEXT,
	"Términos objetivo" TEXT,
	"Por qué es ganable" TEXT,
	"Idioma" TEXT, -- one of ["EN", "ES", "EN+ES"]
	"Evidencia" TEXT,
	"Estado" TEXT, -- one of ["idea", "en cola", "brief hecho", "publicado"]
	"Público" TEXT,
	"Idea" TEXT
)
</sqlite-table>
</parent-data-source-schema>