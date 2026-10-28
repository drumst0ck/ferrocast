Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb81b283cbc7917478e002 as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb81b283cbc7917478e002">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"EN: Apple Support ([support.apple.com/guide/personal-safety](http://support.apple.com/guide/personal-safety)), Macworld, [exifvoid.com](http://exifvoid.com) (web), [latheapp.com](http://latheapp.com) (app de pago). ES: [soydemac.com](http://soydemac.com), [anonshot.com/es](http://anonshot.com/es), [apple.teveotecno.com.ar](http://apple.teveotecno.com.ar), [tecnobits.com](http://tecnobits.com). Todas centradas en fotos; casi nadie cubre vídeo, audio o PDF. Ferrocast tiene /tools/edit-metadata solo en inglés. (2026-10-28, búsqueda pública)","Idea":"Tarea /remove-metadata-mac + /es/quitar-metadatos-mac: \"Quitar ubicación y metadatos de fotos, vídeos, audio y PDF en Mac antes de compartir\" (Fotos/Vista Previa vs Ferrocast Edit metadata).","Idioma":"EN+ES","Intención":"Tarea + pregunta informativa (también a asistentes de IA)","Por qué es ganable":"Apple y Macworld dominan la parte de fotos, pero ninguna página resuelve vídeo, audio, GIF y PDF en el mismo sitio, que es lo que hace Ferrocast (y sin pérdida en JPEG/PNG). Menos cerca de instalar, por eso prioridad media. Honesto: 'Remove all' deja algunas etiquetas de contenedor.","Prioridad":3,"Público":"Gente preocupada por la privacidad, periodistas, vendedores en marketplaces","Términos objetivo":"remove exif data mac; remove location from photo mac; remove metadata from pdf mac; strip metadata from video mac; quitar metadatos foto mac; eliminar ubicación de una foto mac","url":"https://app.notion.com/p/4073a474adbb81b283cbc7917478e002"}
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