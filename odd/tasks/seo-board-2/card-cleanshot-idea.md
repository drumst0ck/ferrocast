Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb81a7980ec0a0e2d3cd96 as of 2026-10-28T12:07:26.261Z:
<page url="https://app.notion.com/p/4073a474adbb81a7980ec0a0e2d3cd96">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"brief hecho","Evidencia":"Rankean un hilo de r/macapps ([reddit.com/r/macapps/comments/1w5wgfu](http://reddit.com/r/macapps/comments/1w5wgfu), sept. 2026, tras CleanShot v5), [notchy.dev/mac-scrolling-screenshot](http://notchy.dev/mac-scrolling-screenshot), [macshot.io](http://macshot.io) y repos de GitHub (8tp/ScreenCap, lzhgus/Capso con licencia BSL). Ferrocast no tiene página de captura salvo /screen-studio-alternative y la doc /docs/launcher/screen-capture. (2026-10-28, búsqueda pública) Brief: <mention url=\"https://app.notion.com/p/4073a474adbb81c89a98d061488dff72\" />","Idea":"Página /cleanshot-alternative: \"Free CleanShot X alternative inside your launcher: area/window/scrolling capture, copy text (OCR), pin, annotate, and screen recording with auto zoom\" (macOS 14+).","Idioma":"EN","Intención":"Comercial / alternativa","Por qué es ganable":"La salida de CleanShot v5 ha reactivado la búsqueda de alternativas gratis y los resultados son apps indie y un hilo de Reddit. Ferrocast junta capturas (incluida con scroll), OCR, pin, anotación y grabación con zoom automático en una app gratis; se enlaza con la página de Screen Studio. Honesto: no hay subida a la nube ni editor tan profundo.","Prioridad":2,"Público":"Devs, diseñadores y creadores que pagan o evalúan CleanShot X","Términos objetivo":"cleanshot x alternative; free cleanshot alternative; open source cleanshot alternative; scrolling screenshot mac; pin screenshot mac; copy text from screenshot mac","url":"https://app.notion.com/p/4073a474adbb81a7980ec0a0e2d3cd96"}
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