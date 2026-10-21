# Ferrocast: estado de las funciones y qué está verificado

Registro de honestidad del proyecto, movido desde el README. Cada fila indica qué está implementado y qué no se ha probado en una sesión real. La web lo presenta como documentación en <https://ferrocast.app/docs/reference/status/> con el vocabulario Tested / Built / Typed (`web/src/data/status.ts`). Actualiza este archivo cuando cambie el estado de una función.

## Estado

| Función | Estado |
|---|---|
| Root Search (apps, comandos, ranking con frecency, alias, favoritos) | Implementado |
| Calculadora (mates, %, unidades), colores, detección de URL | Implementado |
| Divisas (tasas BCE vía frankfurter.dev, caché 12 h) y husos horarios (`time in tokyo`, `5pm ldn in sf`) | Implementado; las divisas requieren red |
| Panel de acciones ⌘K y atajos | Implementado |
| Historial del portapapeles (texto, archivos, imágenes, pin) | Implementado; ignora marcas de gestores de contraseñas |
| Snippets con placeholders y auto-expansión | Implementado; necesita permisos |
| Quicklinks con argumentos | Implementado |
| Script Commands compatibles con Raycast | Implementado (full, compact, silent, inline) |
| Gestión de ventanas (~40 comandos) | Implementado; necesita Accesibilidad |
| Comandos de sistema, emoji, archivos (Spotlight), cambiar ventana, notas | Implementado |
| Kill Process (procesos por CPU y memoria, SIGTERM o SIGKILL con ⌘↵) | Implementado |
| Temporizadores (`timer 10m`, `pomodoro`; aviso y sonido al terminar; vista Timers con cancelación) | Implementado |
| Toggle Wi-Fi y Toggle Keep Awake (caffeinate) como comandos de sistema | Implementado; no probados en sesión real |
| Quick Look de archivos (⌘Y) y «Check for Updates» vía Homebrew | Implementado; no probados en sesión real |
| Calculator History (últimos 100 cálculos, borrar entrada o todo), emoji recientes, «Look up in Dictionary» como fallback | Implementado |
| Icono en barra de menú, ajustes, hotkeys por comando | Implementado |
| Emoji en color (renderizados con AppKit) | Implementado |
| Ask AI con streaming y conversación. Cuatro proveedores: servidor compatible con OpenAI (Ollama por defecto, LM Studio, OpenAI…), **Sign in with ChatGPT** (mismo flujo OAuth y backend que Codex CLI; usa tu plan de ChatGPT; zona gris de términos, avisado en la app), API de Anthropic (clave propia) y **Claude Code** (la CLI oficial, con tu suscripción a Claude). Claves en el Keychain | Implementado; OpenAI probado contra un servidor simulado, Claude Code probado en real (`cargo test claude_code_live -- --ignored`), Anthropic API sin probar en real |
| Arranque al iniciar sesión (LaunchAgent) | Implementado; se aplica en el siguiente inicio de sesión |
| Bienvenida y permisos, atajos, Acerca de | Implementado |
| Exportar copia de seguridad (sin historial del portapapeles), importar snippets (JSON de Raycast) | Implementado |
| Instancia única, log de errores (`Reveal Log File`) | Implementado |
| Icono, entitlements, DMG, fórmula de Homebrew | Implementado; el script de notarización es opcional y no se ha ejecutado |
| Conversión de archivos: los 46 formatos de entrada y los 275 pares de la tabla de Tangerine (imagen, audio, vídeo, documentos, hojas, PDF, presentaciones, EPUB, subtítulos, archivos comprimidos); herramientas de macOS primero, ffmpeg, LibreOffice y cwebp opcionales | Implementado; los 275 pares se ejecutan sobre archivos reales generados y se lee el resultado (texto dentro del documento, ancho de imagen, duración de audio/vídeo). No demuestra fidelidad de maquetación; RAR solo con un fichero sintético, porque el `unrar` de este Mac es x86; `.pages` escrito en formato iWork '09 y abierto solo con LibreOffice, no con Pages |
| Herramientas de imagen: comprimir, redimensionar, recortar, quitar metadatos, PDF combinado, rotar, voltear, fondo (color o copia desenfocada, margen, esquinas, sombra: `frame blur 40`), collage (cuadrícula, fila, columna, destacado: `collage row 1600 gap 16`), ajustes (`edit brightness 20 grain 30 dehaze 50`), redactar caras/texto (Vision) o áreas escritas con color o desenfoque (`redact 10,10 200x100 blur`), anotar (lienzo propio; con varias imágenes, Vista Previa) | Implementado como órdenes escritas, probado con archivos reales; la vista previa en vivo y el lienzo están en las filas siguientes (⇧⌘A, ⇧⌘E, ⇧⌘K); otra imagen como fondo con `frame image /ruta/fondo.jpg`; caras: sin prueba con foto real de una cara |
| Acciones de archivo (copiar, copiar ruta, mover a…, Terminal/iTerm, AirDrop, comprimir en ZIP) y File Shelf | Implementado; «Zip» es una acción añadida a petición (no figura entre las seis de Tangerine): cada archivo o carpeta seleccionado genera su propio .zip al lado, sin sobrescribir, y el nombre pierde la extensión original (`foto.png` pasa a `foto.zip`); AirDrop, Terminal y Zip no probados en sesión real |
| Rueda al arrastrar archivos (Shift: conversiones, Control: acciones) | Implementado; dibujo verificado por captura, el gesto real sin probar |
| Estantería flotante (panel nativo siempre encima, anclado a un borde, arrastrar dentro y fuera) | Implementado; no probado en sesión real |
| Rueda de herramientas (Command), compresión por tamaño, recorte exacto, metadatos EXIF/GPS, recortar/dividir/unir | Implementado; probado con archivos reales |
| Herramientas de audio y vídeo (ffmpeg): silenciar, velocidad, fotograma, recortar encuadre, tapar con caja o desenfoque, unir clips (silencio generado en los mudos), normalizar volumen, visualizador (onda o imagen fija), quitar silencios de los extremos, mono/estéreo, pitido; organizar páginas de PDF (`pages 3,1,2r`) | Implementado como órdenes escritas; probado con archivos reales y revisado por un verificador independiente sin bloqueantes. Normalizar es de una pasada y el aviso muestra la sonoridad medida antes y después (LUFS), sin forma de onda; WMV falla en silenciar/recortar encuadre; sin editor visual ni forma de onda |
| Rueda de formatos desde un archivo con el teclado (⌥↵ en un resultado; Shift/Command/Control cambian de rueda) | Implementado; sin prueba de teclas en sesión real. Intro sobre un archivo sigue abriéndolo; Intro sobre la selección de Finder, no incluido |
| Organizador visual de PDF (⇧⌘O sobre un PDF: miniaturas, girar, duplicar, quitar y reordenar páginas, guarda una copia) | Implementado; dibujo verificado por captura, teclas sin probar en sesión real; arrastrar las miniaturas para reordenar probado solo con eventos sintéticos (el gesto real y el desplazamiento automático con PDF largos sin probar; no se probó el PDF que escribe tras arrastrar, solo el plan) |
| Lienzo de anotación propio (⇧⌘A sobre una imagen: flecha, línea, caja, elipse, resaltado, lápiz, etiquetas numeradas y lupa; guarda una copia PNG a tamaño completo) | Implementado; dibujo verificado por captura y guardado con archivo real; ratón y teclas sin probar en sesión real; con texto libre (fuente Arial del sistema); con bocadillos con puntero e imágenes superpuestas (selector de archivos, sin probar en sesión real); «Anotar» del menú de herramientas abre el lienzo con una imagen y Vista Previa con varias |
| Editor de fotos con vista previa en vivo (⇧⌘E: nueve deslizadores, copia a tamaño completo con ↵) | Implementado; dibujo verificado por captura, deslizadores sin probar en sesión real; la vista previa usa una copia reducida |
| Editor de recorte (⇧⌘K: rectángulo arrastrable con asas, proporciones fijas, ancho y alto exactos; copia con ↵) | Implementado; dibujo verificado por captura y recorte con archivo real; arrastre sin probar en sesión real |
| Editor de redacción (⇧⌘R: arrastrar para añadir áreas, moverlas y cambiar su tamaño, color sólido o desenfoque; copia con ↵) | Implementado; dibujo verificado por captura, arrastre sin probar en sesión real; la vista previa solo marca las áreas |
| Editor de fondo (⇧⌘B: margen, esquinas y sombra con deslizadores, fondo de color o copia desenfocada; copia con ↵) | Implementado; dibujo verificado por captura, deslizadores sin probar en sesión real; fondo de color, copia desenfocada u otra imagen elegida con «Choose image…»; sin probar el diálogo nativo de archivos en sesión real |
| Editor de collage (con 2 o más imágenes: cuadrícula, fila, columna o destacado; ancho, separación, esquinas y fondo; copia con ↵) | Implementado; dibujo verificado por captura, deslizadores sin probar en sesión real; las imágenes se reordenan arrastrando sus miniaturas (solo eventos sintéticos, el gesto real sin probar) |
| Visor de recorte de vídeo (⇧⌘V: fotograma en el cursor, marcadores de inicio y fin, paso por fotograma o segundo, corte exacto recodificado con ↵; requiere ffmpeg) | Implementado; dibujo verificado por captura con un fotograma real, teclas y barra sin probar en sesión real; latencia de la vista previa sin medir |
| Ventana de recorte de encuadre de vídeo (⇧⌘J: fotograma real, rectángulo arrastrable, proporciones, ancho y alto exactos; copia con ↵; requiere ffmpeg) | Implementado; dibujo verificado por captura, arrastre sin probar en sesión real; el fotograma de referencia es fijo (10 % de la duración) |
| Ventana de velocidad de vídeo (⇧⌘L: 0,25–4×, presets y duración resultante; requiere ffmpeg) | Implementado; captura y tests de renderizado verificados; interacción del deslizador sin probar en sesión real; no reproduce el vídeo |
| Visor de fotogramas de vídeo (⇧⌘S: barra temporal, paso por fotograma o segundo, ↵ guarda PNG a resolución original) | Implementado; tests de eventos y captura verificados; interacción en sesión real y exactitud con vídeo de frecuencia variable sin probar |
| Ventana de redacción de vídeo con varias zonas (acción Redact Video…): lista de zonas, cada una con su rectángulo arrastrable, su rango de tiempo en segundos y negro sólido o desenfoque; control de fotograma de referencia (también en el recorte de encuadre); copia con ↵; requiere ffmpeg | Implementado; ejecutor probado con vídeo real (píxeles, zonas solapadas, vídeo rotado) y ventana verificada por captura y eventos sintéticos; arrastre y teclas sin probar en sesión real; la vista previa solo marca las zonas; fotograma de referencia sin ejecución real contra ffmpeg ni latencia medida |
| Rueda con diseño nuevo ("halo"): acento por rueda (naranja, azul, verde), sector resaltado con degradado y brillo, centro con nombre de la rueda, recuento y tipo, todos los segmentos con su nombre bajo el icono (desde 9 la letra es más pequeña y los nombres largos se cortan con «…»; el nombre completo sale en el centro al pasar por encima); la rueda de acciones tiene las seis acciones de Tangerine más Zip y la de conversión muestra todas las salidas (las que necesitan una herramienta no instalada, atenuadas, con la orden de instalación) | Implementado; dibujo verificado por captura en oscuro y claro; sin animaciones (no hechas), gesto real, transparencia real y Retina sin probar |
| Herramientas de la rueda y de Convertir según el formato (tabla de 26 herramientas); las que no tienen ventana usan presets o comandos escritos | Implementado; probado con eventos sintéticos (flechas, Esc, cambio de rueda), no con teclas reales |
| Extraer archivos (zip, tar, tar.gz, gz y rar) en una carpeta nueva junto al archivo, tras comprobar que no hay rutas absolutas ni `..` | Implementado y revisado por un verificador independiente; RAR solo probado con un archivo sintético leído con `tar`; los enlaces simbólicos los resuelven `tar` y `ditto`; `gz` sin límite de tamaño |
| Elegir el orden al unir PDF, audio o vídeo y al crear un PDF desde imágenes (ventana "Choose Order") | Implementado (teclas ⌘↑/⌘↓ y arrastrar filas; el arrastre probado solo con eventos sintéticos); Intro ejecutando la unión no probado |
| Dividir vídeo y audio: ventana con marcadores (M, Retroceso), partes iguales, todos los clips en una carpeta nueva | Implementado; ejecutor probado con vídeo y audio reales; ventana sin prueba con ratón y teclado reales; WMV, vídeo de frecuencia variable y cortes sin recodificar sin probar |
| Comprimir: imágenes por tamaño, porcentaje o preset con reducción de tamaño; audio y vídeo con ffmpeg (tasa de bits calculada); PDF con Ghostscript opcional y solo presets | Implementado; la estimación de tamaño es aproximada, no una codificación de muestra; PDF requiere Ghostscript (sin vía nativa demostrada); calidad y archivos grandes sin probar |
| Editar metadatos de audio, vídeo, GIF y PDF (ventana por archivo, pista y capítulo; quitar todo) | Implementado y probado con archivos reales; capítulos solo lectura; un GIF solo admite quitar; "quitar todo" deja algunas etiquetas del contenedor; imágenes siguen con comandos escritos; ventana sin prueba con ratón y teclado reales |
| Recortar audio con forma de onda (ventana "Trim Audio": marcadores, cajas de inicio y fin exactos, botón "Trim silence"), normalizar (objetivo en LUFS, rango y pico real, con medición previa) canales (mono o estéreo, volumen izquierdo y derecho en dB), pitido con varios rangos (ventana «Bleep Audio»: lista de rangos sobre la onda, cada uno sustituido por un tono de 1 kHz) y audio a vídeo (ventana «Audio to Video»: onda o imagen fija, horizontal, vertical o cuadrado) | Implementado; extractor de muestras, normalización, pitido (medido con Goertzel) y audio a vídeo probados con audio real; ventanas sin prueba con ratón y teclado reales; el pitido y el audio a vídeo solo se ofrecen para un archivo de audio (el ejecutor del pitido rechaza vídeo); el diálogo nativo de imagen de audio a vídeo sin probar; el umbral de silencio es por muestra (un chasquido lo desplaza); sin barra de comparación antes y después |
| Página web `/files` (rueda interactiva, explorador de las 275 conversiones con filtros, herramientas con insignias de estado honestas) | Generada desde los datos de la app y comprobada en un navegador local; sin publicar; sin capturas de las ventanas nuevas |
| Extensiones React, sync, Search Menu Bar Items, contexto de IA (selección/pantalla) | No incluido |


## Distribución

La vía soportada es Homebrew compilando desde fuente (arriba). El DMG de `scripts/dmg.sh` **sin notarizar**
lo bloqueará Gatekeeper en otros Macs que lo descarguen; solo sirve para uso propio. La notarización
(`scripts/notarize.sh`) es opcional y requiere cuenta de desarrollador de pago.

## Qué está verificado y qué no

- Verificado: tests (núcleo, bindings, estado de la UI), arranque del `.app`, y el aspecto real de la
  ventana mediante capturas internas (`scripts/shot.sh`): búsqueda, calculadora, divisas, husos,
  portapapeles con detalle, panel ⌘K, emoji y ajustes.
- **No verificado en sesión real**: foco al abrir/cerrar, hotkeys globales, pegado, expansión de
  snippets y gestión de ventanas (requieren Accesibilidad, que concede el usuario).
- Permisos (Accesibilidad / Input Monitoring) los concede el usuario. Al recompilar con firma
  ad-hoc macOS puede pedirlos de nuevo. Para distribuir hace falta firma Developer ID y notarización.
- Los scripts con `needsConfirmation` piden confirmación antes de ejecutarse.
- Los argumentos de Script Commands (incluidos los de tipo `password`) se pasan por línea de comandos,
  como en Raycast, y pueden verse con `ps` mientras el script corre. No pongas secretos críticos ahí.
