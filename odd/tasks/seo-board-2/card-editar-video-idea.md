Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb81d680aef7b0ab4b168d as of 2026-10-28T11:23:49.938Z:
<page url="https://app.notion.com/p/4073a474adbb81d680aef7b0ab4b168d">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"idea","Evidencia":"En español rankean páginas en inglés: [apps.digitalcoding.com/software/dc-studio](http://apps.digitalcoding.com/software/dc-studio), [video-tools.mytory.net](http://video-tools.mytory.net), [github.com/jub0t/Concat](http://github.com/jub0t/Concat), y herramientas online ([montto.io/tools](http://montto.io/tools), [joclip.com/tools](http://joclip.com/tools)). Para 'speed up video mac' en inglés: [samirpatil2000.github.io/products/tempo](http://samirpatil2000.github.io/products/tempo), [hangar.tools](http://hangar.tools), [onemacapp.com](http://onemacapp.com), [crispvideo.app](http://crispvideo.app). Las páginas /tools/\\* de Ferrocast existen solo en inglés y como plantilla. (2026-10-28, búsqueda pública)","Idea":"Tarea /es/editar-video-mac-sin-imovie: \"Recortar, dividir, unir, acelerar y quitar el sonido a un vídeo en Mac sin iMovie y sin subirlo\" (versión ES de /tools/trim-video, split-video, join-videos, video-speed, mute-video, crop-video).","Idioma":"ES","Intención":"Tarea / transaccional","Por qué es ganable":"SERP en español sin contenido nativo y con webs que obligan a subir el vídeo. Ferrocast cubre las seis tareas desde el mismo menú (velocidad 0,25x–4x manteniendo el tono). Ojo: requiere ffmpeg; decirlo con el comando brew.","Prioridad":2,"Público":"Usuarios hispanohablantes que quieren ediciones rápidas sin aprender iMovie ni usar webs","Términos objetivo":"recortar video mac; unir videos mac; acelerar video mac; quitar sonido a un video mac; dividir video mac; editar video mac sin imovie","url":"https://app.notion.com/p/4073a474adbb81d680aef7b0ab4b168d"}
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