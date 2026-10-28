Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb819fafb7e5dd9e84c4b0 as of 2026-10-28T12:07:19.887Z:
<page url="https://app.notion.com/p/4073a474adbb819fafb7e5dd9e84c4b0">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"brief hecho","Evidencia":"EN: rankean [typefire.app](http://typefire.app) (landing de vendedor), [github.com/dkdannyboy/snipkey](http://github.com/dkdannyboy/snipkey), [marcotteink.github.io/shorthand](http://marcotteink.github.io/shorthand), [github.com/afomera/expandly](http://github.com/afomera/expandly) y [lifetimemac.com/apps/espanso](http://lifetimemac.com/apps/espanso). ES: la misma [typefire.app](http://typefire.app) en inglés, [espanso.org](http://espanso.org) y [timingapp.com/blog](http://timingapp.com/blog) (inglés); no aparece contenido nativo en español. Ferrocast solo tiene /docs/launcher/snippets (doc corta). (2026-10-28, búsqueda pública) Brief: <mention url=\"https://app.notion.com/p/4073a474adbb813fac6ac5f5b3a44d54\" /> (ojo: la sync por iCloud aún no está publicada; en la página, 'llega en la próxima versión')","Idea":"Página /text-expander-mac + /es/expansor-de-texto-mac: \"Expansor de texto gratis para Mac dentro del launcher: alternativa a TextExpander\" (snippets con \\{date\\}, \\{clipboard\\}, \\{cursor\\}, snippets anidados, importación desde Raycast JSON, sync opcional por iCloud). Honesto: no hay formularios fill-in.","Idioma":"EN+ES","Intención":"Comercial / alternativa","Por qué es ganable":"Resultados de proyectos pequeños y landings de vendedor; en español el hueco es total. Ángulo propio: snippets + portapapeles + launcher en una app gratis y open source, con importación directa del JSON de Raycast.","Prioridad":2,"Público":"Soporte, ventas, devs y escritores que repiten textos; usuarios que vienen de TextExpander o Raycast","Términos objetivo":"textexpander alternative free mac; free text expander mac; raycast snippets alternative; text expansion mac open source; expansor de texto mac gratis; alternativa a textexpander; snippets mac gratis","url":"https://app.notion.com/p/4073a474adbb819fafb7e5dd9e84c4b0"}
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