Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb81e9bf18d0b09f40f25d as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb81e9bf18d0b09f40f25d">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"La búsqueda en español devuelve páginas en inglés: [textexpander.com/blog/alfred-alternatives](http://textexpander.com/blog/alfred-alternatives), [purplelink.llc/guides](http://purplelink.llc/guides), [github.com/ekinertac/walter](http://github.com/ekinertac/walter), [walterlauncher.com/vs-alfred](http://walterlauncher.com/vs-alfred), [github.com/GeoWizard4645/caduceus](http://github.com/GeoWizard4645/caduceus). Ferrocast tiene /alfred-alternative solo en inglés. (2026-10-28, búsqueda pública)","Idea":"Página /es/alternativa-a-alfred: \"Alternativa gratis a Alfred sin Powerpack\" (versión ES de /alfred-alternative).","Idioma":"ES","Intención":"Comercial / alternativa","Por qué es ganable":"Sin contenido nativo en español y la página EN ya existe. Prioridad media porque Alfred parece tener menos público hispano que Raycast (impresión cualitativa, sin datos).","Prioridad":3,"Público":"Usuarios hispanohablantes de Alfred sin Powerpack","Términos objetivo":"alternativa a alfred; alfred powerpack gratis; alfred o raycast; launcher mac gratis","url":"https://app.notion.com/p/4073a474adbb81e9bf18d0b09f40f25d"}
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