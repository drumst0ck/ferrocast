Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb81078a22f6adf21c4521 as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb81078a22f6adf21c4521">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"En español rankean solo páginas en inglés: [github.com/kumayu0108/litecast](http://github.com/kumayu0108/litecast), [getthaw.app/floe](http://getthaw.app/floe), la extensión de Ollama en [raycast.com/store](http://raycast.com/store) y [ainto.app](http://ainto.app). Ferrocast tiene /local-ai-launcher solo en inglés. (2026-10-28, búsqueda pública)","Idea":"Página /es/ia-local-mac: \"Launcher con IA local en Mac: Ollama, LM Studio y Claude Code con un atajo\" (versión ES de /local-ai-launcher; alternativa gratis a Raycast AI sin Pro).","Idioma":"ES","Intención":"Comercial / alternativa","Por qué es ganable":"Hueco en español y tema de moda; Ferrocast usa Ollama por defecto, guarda claves en el Llavero y no cobra Pro. Página EN existente para adaptar.","Prioridad":2,"Público":"Devs y usuarios técnicos hispanohablantes interesados en privacidad e IA local","Términos objetivo":"ollama mac launcher; ia local mac; usar ollama con atajo mac; alternativa a raycast ai gratis; lm studio mac atajo","url":"https://app.notion.com/p/4073a474adbb81078a22f6adf21c4521"}
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