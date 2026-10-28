Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb8145b395c23c8e9d6dcd as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb8145b395c23c8e9d6dcd">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"Rankean [getpaperlock.com/es](http://getpaperlock.com/es) (web), [es.mac-os.org](http://es.mac-os.org), Soporte de Apple en español (Vista Previa), [pdfsam.org](http://pdfsam.org) y [fyllyo.com](http://fyllyo.com). En EN 'merge PDF on Mac offline' lo domina PDFsam ([pdfsam.org](http://pdfsam.org), [formulae.brew.sh](http://formulae.brew.sh)), por eso solo ES. Ferrocast tiene /tools/merge-pdf, split-pdf y organize-pdf solo en inglés. (2026-10-28, búsqueda pública)","Idea":"Tarea /es/unir-pdf-mac: \"Unir, separar y reordenar páginas de PDF en Mac sin subirlos\" (merge, split y organize PDF; comprimir PDF con Ghostscript).","Idioma":"ES","Intención":"Tarea","Por qué es ganable":"Resultados en español flojos salvo Apple; Vista Previa ya lo hace gratis, así que el ángulo es rapidez (arrastrar al menú, ventana de orden, organizador visual con ⇧⌘O) y todo local. Prioridad media por la competencia de Vista Previa.","Prioridad":3,"Público":"Estudiantes y oficina hispanohablantes","Términos objetivo":"unir pdf mac; combinar pdf mac sin subir; separar páginas pdf mac; reordenar páginas pdf mac; comprimir pdf mac","url":"https://app.notion.com/p/4073a474adbb8145b395c23c8e9d6dcd"}
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