# Ferrocast: estado de las funciones

Registro del estado de las funciones, movido desde el README. Cada fila indica si la función está probada (**Tested**) o es solo de orden escrita (**Typed**), con sus límites reales. La web lo presenta como documentación en <https://ferrocast.app/docs/reference/status/> con el vocabulario Tested / Typed (`web/src/data/status.ts`). Actualiza este archivo cuando cambie el estado de una función.

## Estado

| Función | Estado | Notas |
|---|---|---|
| Root Search (apps, comandos, ranking con frecency, alias, favoritos) | Tested | |
| Calculadora (mates, %, unidades), colores, detección de URL | Tested | |
| Divisas (tasas BCE vía frankfurter.dev, caché 12 h) y husos horarios (`time in tokyo`, `5pm ldn in sf`) | Tested | Las divisas requieren red |
| Panel de acciones ⌘K y atajos | Tested | |
| Historial del portapapeles (texto, archivos, imágenes, pin) | Tested | Ignora marcas de gestores de contraseñas |
| Snippets con placeholders y auto-expansión | Tested | Necesita permisos |
| Quicklinks con argumentos | Tested | |
| Script Commands compatibles con Raycast | Tested | Modos full, compact, silent, inline |
| Gestión de ventanas (~40 comandos) | Tested | Necesita Accesibilidad |
| Comandos de sistema, emoji, archivos (Spotlight), cambiar ventana, notas | Tested | |
| Kill Process (procesos por CPU y memoria, SIGTERM o SIGKILL con ⌘↵) | Tested | |
| Temporizadores (`timer 10m`, `pomodoro`; aviso y sonido al terminar; vista Timers con cancelación) | Tested | |
| Toggle Wi-Fi y Toggle Keep Awake (caffeinate) como comandos de sistema | Tested | |
| Quick Look de archivos (⌘Y) y «Check for Updates» vía Homebrew | Tested | |
| Calculator History (últimos 100 cálculos, borrar entrada o todo), emoji recientes, «Look up in Dictionary» como fallback | Tested | |
| Icono en barra de menú, ajustes, hotkeys por comando | Tested | |
| Emoji en color (renderizados con AppKit) | Tested | |
| Ask AI con streaming y conversación. Cuatro proveedores: servidor compatible con OpenAI (Ollama por defecto, LM Studio, OpenAI…), **Sign in with ChatGPT** (mismo flujo OAuth y backend que Codex CLI; usa tu plan de ChatGPT; zona gris de términos, avisado en la app), API de Anthropic (clave propia) y **Claude Code** (la CLI oficial, con tu suscripción a Claude). Claves en el Keychain | Tested | Funciona con el servidor compatible con OpenAI, Claude Code y la API de Anthropic (clave propia) |
| Arranque al iniciar sesión (LaunchAgent) | Tested | Se aplica en el siguiente inicio de sesión |
| Bienvenida y permisos, atajos, Acerca de | Tested | |
| Exportar copia de seguridad (sin historial del portapapeles), importar snippets (JSON de Raycast) | Tested | |
| Instancia única, log de errores (`Reveal Log File`) | Tested | |
| Icono, entitlements, DMG, fórmula de Homebrew | Tested | El script de notarización es opcional |
| Conversión de archivos: los 46 formatos de entrada y los 275 pares de la tabla de Tangerine (imagen, audio, vídeo, documentos, hojas, PDF, presentaciones, EPUB, subtítulos, archivos comprimidos); herramientas de macOS primero, ffmpeg, LibreOffice y cwebp opcionales | Tested | Los 275 pares se ejecutan sobre archivos reales generados y se lee el resultado (texto dentro del documento, ancho de imagen, duración de audio/vídeo). La extracción de RAR necesita un `unrar` operativo; `.pages` se escribe en formato iWork '09 |
| Herramientas de imagen: comprimir, redimensionar, recortar, quitar metadatos, PDF combinado, rotar, voltear, fondo (color o copia desenfocada, margen, esquinas, sombra: `frame blur 40`), collage (cuadrícula, fila, columna, destacado: `collage row 1600 gap 16`), ajustes (`edit brightness 20 grain 30 dehaze 50`), redactar caras/texto (Vision) o áreas escritas con color o desenfoque (`redact 10,10 200x100 blur`), anotar (lienzo propio; con varias imágenes, Vista Previa) | Typed | Órdenes escritas; la vista previa en vivo y el lienzo están en las filas siguientes (⇧⌘A, ⇧⌘E, ⇧⌘K); otra imagen como fondo con `frame image /ruta/fondo.jpg` |
| Acciones de archivo (copiar, copiar ruta, mover a…, Terminal/iTerm, AirDrop, comprimir en ZIP) y File Shelf | Tested | «Zip» es una acción añadida a petición (no figura entre las seis de Tangerine): cada archivo o carpeta seleccionado genera su propio .zip al lado, sin sobrescribir, y el nombre pierde la extensión original (`foto.png` pasa a `foto.zip`) |
| Rueda al arrastrar archivos (Shift: conversiones, Control: acciones) | Tested | |
| Estantería flotante (panel nativo siempre encima, anclado a un borde, arrastrar dentro y fuera) | Tested | |
| Rueda de herramientas (Command), compresión por tamaño, recorte exacto, metadatos EXIF/GPS, recortar/dividir/unir | Tested | |
| Herramientas de audio y vídeo (ffmpeg): silenciar, velocidad, fotograma, recortar encuadre, tapar con caja o desenfoque, unir clips (silencio generado en los mudos), normalizar volumen, visualizador (onda o imagen fija), quitar silencios de los extremos, mono/estéreo, pitido; organizar páginas de PDF (`pages 3,1,2r`) | Typed | Órdenes escritas. Normalizar es de una pasada y el aviso muestra la sonoridad medida antes y después (LUFS), sin forma de onda; WMV falla en silenciar/recortar encuadre; sin editor visual ni forma de onda |
| Rueda de formatos desde un archivo con el teclado (⌥↵ en un resultado; Shift/Command/Control cambian de rueda) | Tested | Intro sobre un archivo sigue abriéndolo. Intro sobre la selección de Finder es un ajuste opcional: Ajustes > General > «Enter on Finder selection» (desactivado por defecto, necesita Accesibilidad), que abre la rueda de conversión en lugar de renombrar; el comando «Convert Finder Selection» también funciona |
| Organizador visual de PDF (⇧⌘O sobre un PDF: miniaturas, girar, duplicar, quitar y reordenar páginas, guarda una copia) | Tested | Las páginas también se reordenan arrastrando; un PDF largo no se puede arrastrar más allá de la tira visible, usa ⌘←/→ |
| Lienzo de anotación propio (⇧⌘A sobre una imagen: flecha, línea, caja, elipse, resaltado, lápiz, etiquetas numeradas y lupa; guarda una copia PNG a tamaño completo) | Tested | Con texto libre (fuente Arial del sistema); con bocadillos con puntero e imágenes superpuestas; «Anotar» del menú de herramientas abre el lienzo con una imagen y Vista Previa con varias |
| Editor de fotos con vista previa en vivo (⇧⌘E: nueve deslizadores, copia a tamaño completo con ↵) | Tested | La vista previa usa una copia reducida |
| Editor de recorte (⇧⌘K: rectángulo arrastrable con asas, proporciones fijas, ancho y alto exactos; copia con ↵) | Tested | |
| Editor de redacción (⇧⌘R: arrastrar para añadir áreas, moverlas y cambiar su tamaño, color sólido o desenfoque; copia con ↵) | Tested | La vista previa solo marca las áreas; el recubrimiento se aplica al guardar |
| Editor de fondo (⇧⌘B: margen, esquinas y sombra con deslizadores, fondo de color o copia desenfocada; copia con ↵) | Tested | Fondo de color, copia desenfocada u otra imagen elegida con «Choose image…» |
| Editor de collage (con 2 o más imágenes: cuadrícula, fila, columna o destacado; ancho, separación, esquinas y fondo; copia con ↵) | Tested | Las imágenes se reordenan arrastrando sus miniaturas |
| Visor de recorte de vídeo (⇧⌘V: fotograma en el cursor, marcadores de inicio y fin, paso por fotograma o segundo, corte exacto recodificado con ↵; requiere ffmpeg) | Tested | Corte exacto recodificado; requiere ffmpeg |
| Ventana de recorte de encuadre de vídeo (⇧⌘J: fotograma real, rectángulo arrastrable, proporciones, ancho y alto exactos; copia con ↵; requiere ffmpeg) | Tested | El fotograma de referencia es fijo (10 % de la duración) |
| Ventana de velocidad de vídeo (⇧⌘L: 0,25–4×, presets y duración resultante; requiere ffmpeg) | Tested | No reproduce el vídeo |
| Visor de fotogramas de vídeo (⇧⌘S: barra temporal, paso por fotograma o segundo, ↵ guarda PNG a resolución original) | Tested | |
| Ventana de redacción de vídeo con varias zonas (acción Redact Video…): lista de zonas, cada una con su rectángulo arrastrable, su rango de tiempo en segundos y negro sólido o desenfoque; control de fotograma de referencia (también en el recorte de encuadre); copia con ↵; requiere ffmpeg | Tested | La vista previa solo marca las zonas |
| Rueda con diseño nuevo ("halo"): acento por rueda (naranja, azul, verde), sector resaltado con degradado y brillo, centro con nombre de la rueda, recuento y tipo, todos los segmentos con su nombre bajo el icono (desde 9 la letra es más pequeña y los nombres largos se cortan con «…»; el nombre completo sale en el centro al pasar por encima); la rueda de acciones tiene las seis acciones de Tangerine más Zip y la de conversión muestra todas las salidas (las que necesitan una herramienta no instalada, atenuadas, con la orden de instalación) | Tested | Sin animaciones |
| Herramientas de la rueda y de Convertir según el formato (tabla de 26 herramientas); las que no tienen ventana usan presets o comandos escritos | Tested | |
| Extraer archivos (zip, tar, tar.gz, gz y rar) en una carpeta nueva junto al archivo, tras comprobar que no hay rutas absolutas ni `..` | Tested | Los enlaces simbólicos los resuelven `tar` y `ditto`; `gz` sin límite de tamaño |
| Elegir el orden al unir PDF, audio o vídeo y al crear un PDF desde imágenes (ventana "Choose Order") | Tested | Teclas ⌘↑/⌘↓ y arrastrar filas |
| Dividir vídeo y audio: ventana con marcadores (M, Retroceso), partes iguales, todos los clips en una carpeta nueva | Tested | |
| Comprimir: imágenes por tamaño, porcentaje o preset con reducción de tamaño; audio y vídeo con ffmpeg (tasa de bits calculada); PDF con Ghostscript opcional y solo presets | Tested | La estimación de tamaño es aproximada, no una codificación de muestra; PDF requiere Ghostscript |
| Editar metadatos de audio, vídeo, GIF y PDF (ventana por archivo, pista y capítulo; quitar todo) | Tested | Capítulos solo lectura; un GIF solo admite quitar; «quitar todo» deja algunas etiquetas del contenedor; imágenes siguen con comandos escritos |
| Recortar audio con forma de onda (ventana "Trim Audio": marcadores, cajas de inicio y fin exactos, botón "Trim silence"), normalizar (objetivo en LUFS, rango y pico real, con medición previa) canales (mono o estéreo, volumen izquierdo y derecho en dB), pitido con varios rangos (ventana «Bleep Audio»: lista de rangos sobre la onda, cada uno sustituido por un tono de 1 kHz) y audio a vídeo (ventana «Audio to Video»: onda o imagen fija, horizontal, vertical o cuadrado) | Tested | El pitido y el audio a vídeo solo se ofrecen para un archivo de audio (el ejecutor del pitido rechaza vídeo); el umbral de silencio es por muestra (un chasquido lo desplaza); sin barra de comparación antes y después |
| Página web `/files` (rueda interactiva, explorador de las 275 conversiones con filtros, herramientas con insignias de estado honestas) | Tested | Generada desde los datos de la app |
| Extensiones React, sync, Search Menu Bar Items, contexto de IA (selección/pantalla) | No incluido |

## Distribución

La vía soportada es Homebrew compilando desde fuente (arriba). El DMG de `scripts/dmg.sh` **sin notarizar**
lo bloqueará Gatekeeper en otros Macs que lo descarguen; solo sirve para uso propio. La notarización
(`scripts/notarize.sh`) es opcional y requiere cuenta de desarrollador de pago.

## Permisos y scripts

- La gestión de ventanas, los hotkeys globales, el pegado y la expansión de snippets requieren Accesibilidad, que concede el usuario.
- Permisos (Accesibilidad / Input Monitoring) los concede el usuario. Al recompilar con firma
  ad-hoc macOS puede pedirlos de nuevo. Para distribuir hace falta firma Developer ID y notarización.
- Los scripts con `needsConfirmation` piden confirmación antes de ejecutarse.
- Los argumentos de Script Commands (incluidos los de tipo `password`) se pasan por línea de comandos,
  como en Raycast, y pueden verse con `ps` mientras el script corre. No pongas secretos críticos ahí.
