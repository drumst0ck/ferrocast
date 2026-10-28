Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb817eba36de91f4952db7 as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb817eba36de91f4952db7">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"ES: [transone.app](http://transone.app), [github.com/madeye/runse-mac](http://github.com/madeye/runse-mac), [matetranslate.com/es](http://matetranslate.com/es) (ayuda de Mate), Soporte de Apple y [frset.com/es-mx/blog](http://frset.com/es-mx/blog) (vendedor). Pocos resultados nativos y casi todos de vendedores. Ferrocast tiene la doc /docs/launcher/translate (EN y ES) pero ninguna página de tarea. (2026-10-28, búsqueda pública)","Idea":"Tarea /translate-selected-text-mac + /es/traducir-texto-seleccionado-mac: \"Traducir texto seleccionado en cualquier app de Mac y reemplazarlo en su sitio, con IA local\".","Idioma":"EN+ES","Intención":"Tarea","Por qué es ganable":"Ángulo único: arrastrar el texto, tocar ⌘ y soltarlo sobre un idioma, y la traducción reemplaza la selección; con Ollama no sale nada del Mac. Prioridad media: hay apps dedicadas y la traducción del sistema.","Prioridad":3,"Público":"Usuarios bilingües, gente que escribe en otro idioma en Slack, correo, etc.","Términos objetivo":"translate selected text mac; translate text in any app mac shortcut; translate with ollama mac; traducir texto seleccionado mac; traducir sin copiar y pegar mac","url":"https://app.notion.com/p/4073a474adbb817eba36de91f4952db7"}
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