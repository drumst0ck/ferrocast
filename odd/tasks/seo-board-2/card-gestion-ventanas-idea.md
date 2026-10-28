Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb81e380bdf323250c279f as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb81e380bdf323250c279f">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"La búsqueda en español devuelve solo repos y landings en inglés: [github.com/Dot-Fun/windowkit](http://github.com/Dot-Fun/windowkit), [nudge.run](http://nudge.run), [github.com/jgaddis99/wrangler](http://github.com/jgaddis99/wrangler), [rectzones.app](http://rectzones.app), [encaje.stevenacz.com](http://encaje.stevenacz.com). Ferrocast tiene /mac-window-management solo en inglés. (2026-10-28, búsqueda pública)","Idea":"Página /es/gestion-de-ventanas-mac: \"Organizar ventanas en Mac con atajos: alternativa gratis a Rectangle y Magnet\" (versión ES de /mac-window-management, con lo que trae Tahoe de serie).","Idioma":"ES","Intención":"Comercial / alternativa + tarea","Por qué es ganable":"Cero contenido nativo en español en los primeros resultados; Ferrocast tiene unos 40 comandos de ventana y la página EN ya existe para adaptar. Además lleva al usuario al launcher completo, no a una app de un solo uso.","Prioridad":2,"Público":"Usuarios hispanohablantes que vienen de Windows o usan monitores grandes","Términos objetivo":"organizar ventanas mac; dividir pantalla mac atajos; alternativa a rectangle; alternativa a magnet gratis; mosaico de ventanas macos tahoe","url":"https://app.notion.com/p/4073a474adbb81e380bdf323250c279f"}
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