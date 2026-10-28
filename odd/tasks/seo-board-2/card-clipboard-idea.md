Here is the result of "fetch" for the Page with URL https://app.notion.com/p/4073a474adbb8187ba28c7a00818ab7c as of 2026-10-28T12:07:22.988Z:
<page url="https://app.notion.com/p/4073a474adbb8187ba28c7a00818ab7c">
<ancestor-path>
<parent-data-source url="collection://6de35012-1676-40c6-a1d7-281ff598db92" name="Tablero de ideas"/>
<ancestor-2-database url="https://app.notion.com/p/c0299291bc8246328ba4bac5b7d45784" title="Tablero de ideas"/>
<ancestor-3-page url="https://app.notion.com/p/4063a474adbb81e99ba6f463a95f393d" title="Ferrocast · SEO/AEO"/>
</ancestor-path>
<properties>
{"Estado":"brief hecho","Evidencia":"Para 'maccy alternative' rankean repos pequeños de GitHub: hasanjafri/Mimer (9 estrellas), mobrava/PasteClip, momenbasel/pesty, samirpatil2000/Buffer, openpaste/openpaste. Para la pregunta de Tahoe y contraseñas: blogs de vendedor ([maccymanager.com](http://maccymanager.com), [clipbook.app](http://clipbook.app), [maccyapp.com](http://maccyapp.com)) y una issue de GitHub (gopasspw/clipboard #11). Ferrocast solo tiene la página ES y la doc /docs/launcher/clipboard. (2026-10-28, búsqueda pública) Brief: <mention url=\"https://app.notion.com/p/4073a474adbb81e989baf8bfc51ab152\" />","Idea":"Página /clipboard-manager-mac: \"Free clipboard manager for Mac that skips passwords: Maccy/Paste alternative, and what Tahoe Spotlight clipboard misses\" (versión EN de /es/historial-portapapeles-mac).","Idioma":"EN","Intención":"Comercial / alternativa + pregunta informativa (Tahoe)","Por qué es ganable":"SERP de repos pequeños y blogs de vendedores. Ferrocast tiene algo concreto: omite a propósito lo copiado desde gestores de contraseñas, guarda texto, archivos e imágenes, fija entradas y sincroniza por iCloud opcional. Honesto: los gestores dedicados tienen pinboards/OCR que Ferrocast no tiene.","Prioridad":2,"Público":"Usuarios de Mac que estrenan Tahoe, usuarios de Paste que no quieren suscripción","Términos objetivo":"maccy alternative; paste app alternative free; free clipboard manager mac; clipboard history mac tahoe; does spotlight clipboard history save passwords; open source clipboard manager mac","url":"https://app.notion.com/p/4073a474adbb8187ba28c7a00818ab7c"}
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