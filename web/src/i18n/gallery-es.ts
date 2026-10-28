/** Spanish names, alts and captions for the files gallery. Window titles stay the English labels on the screenshots. */
export const galleryEs: Record<string, { name: string; alt: string; caption: string }> = {
  'wheel-convert': {
    name: 'Rueda Convertir (oscuro)',
    alt: 'La rueda de conversión en el tema oscuro para un archivo PNG, con segmentos JPEG, WebP, HEIC, TIFF, AVIF, BMP y PDF, y WebP resaltado',
    caption: 'La rueda Convertir para un PNG: un segmento por formato de destino, y el resaltado con su nombre en el centro.',
  },
  'wheel-tools': {
    name: 'Rueda Herramientas (oscuro)',
    alt: 'La rueda de herramientas en el tema oscuro para un archivo PNG, con segmentos Compress, Metadata, Edit Photo, Annotate, Background, Crop y Redact',
    caption: 'La rueda Herramientas para una imagen: cada segmento abre una ventana o ejecuta un ajuste.',
  },
  'wheel-tools-video': {
    name: 'Rueda Herramientas para un vídeo (oscuro)',
    alt: 'La rueda de herramientas en el tema oscuro para un vídeo MP4, con nueve segmentos: Compress, Metadata, Mute, Trim, Crop Video, Speed, Snapshots, Split y Redact Video',
    caption: 'La rueda Herramientas para un vídeo tiene nueve segmentos, cada uno con su nombre bajo el icono. Los nombres largos se acortan para caber en el anillo.',
  },
  'wheel-actions': {
    name: 'Rueda Acciones (oscuro)',
    alt: 'La rueda de acciones en el tema oscuro con siete segmentos: AirDrop, Float, Copy, Move to, Copy Path, Terminal y Zip',
    caption: 'La rueda Acciones: AirDrop, Float shelf, Copy, Move to, Copy Path, Terminal y Zip.',
  },
  'wheel-convert-light': {
    name: 'Rueda Convertir (claro)',
    alt: 'La rueda de conversión en el tema claro para un archivo PNG, con WebP resaltado',
    caption: 'La misma rueda Convertir en el tema claro.',
  },
  vtrim: {
    name: 'Trim Video',
    alt: 'La ventana Trim Video con un fotograma de patrón de prueba, marcas de inicio y fin en una línea de tiempo y la duración del clip',
    caption: 'Trim Video: un fotograma de vista previa, marcas de inicio y fin en una línea de tiempo, y la duración del clip.',
  },
  vcrop: {
    name: 'Crop Video',
    alt: 'La ventana Crop Video con un recorte cuadrado sobre un fotograma de patrón de prueba y proporciones predefinidas',
    caption: 'Crop Video: un recuadro de recorte sobre un fotograma de vista previa, con proporciones predefinidas y ancho y alto exactos.',
  },
  vspeed: {
    name: 'Change Video Speed',
    alt: 'La ventana Change Video Speed con ajustes de 0.25× a 4× y una lectura de duración de 6 segundos a 3 segundos',
    caption: 'Change Video Speed: ajustes o un control deslizante, con la nueva duración a la vista.',
  },
  vsnapshot: {
    name: 'Video Snapshots',
    alt: 'La ventana Video Snapshots con un fotograma de patrón de prueba y una línea de tiempo',
    caption: 'Video Snapshots: avanza hasta un fotograma y guárdalo como PNG a resolución completa.',
  },
  vsplit: {
    name: 'Split Video',
    alt: 'La ventana Split Video con dos marcas de corte en una línea de tiempo que dan tres clips',
    caption: 'Split Video: marcas de corte en una línea de tiempo, o partes iguales cuando no hay cortes.',
  },
  vredact: {
    name: 'Redact Video',
    alt: 'La ventana Redact Video con tres zonas de cobertura numeradas sobre un fotograma de patrón de prueba, una lista de zonas con intervalos, modos Solid y Blur y un control Reference frame',
    caption: 'Redact Video: varias zonas, cada una con su inicio, su fin y una cobertura sólida o desenfocada. La vista previa solo marca las zonas; la cobertura se aplica al guardar la copia.',
  },
  atrim: {
    name: 'Trim Audio',
    alt: 'La ventana Trim Audio con una forma de onda, un tramo seleccionado de 1 a 4 segundos y un botón Trim silence',
    caption: 'Trim Audio: una forma de onda con un tramo seleccionado, tiempos de inicio y fin escritos, y un botón Trim silence.',
  },
  anorm: {
    name: 'Normalize Audio',
    alt: 'La ventana Normalize Audio con la sonoridad medida, ajustes de destino y campos de rango de sonoridad y pico real',
    caption: 'Normalize Audio: sonoridad medida, ajustes para broadcast, podcast y música, y campos de rango y pico.',
  },
  achan: {
    name: 'Audio Channels',
    alt: 'La ventana Audio Channels con las opciones Mono y Stereo y campos de volumen izquierdo y derecho',
    caption: 'Audio Channels: Mono o Stereo, con un volumen en dB para cada lado.',
  },
  avis: {
    name: 'Audio to Video',
    alt: 'La ventana Audio to Video para un MP3, con opciones Waveform y Still image y formas Landscape, Portrait y Square, y Portrait seleccionado',
    caption: 'Audio to Video: una forma de onda en movimiento o una imagen fija, en un lienzo horizontal, vertical o cuadrado. La ventana muestra los ajustes elegidos, no una vista previa del vídeo.',
  },
  'avis-light': {
    name: 'Audio to Video (claro)',
    alt: 'La ventana Audio to Video en el tema claro, con las mismas opciones Waveform y Portrait en tarjetas agrupadas',
    caption: 'La misma ventana en el tema claro, para mostrar que las tarjetas agrupadas y las teclas del pie se leen en los dos temas.',
  },
  bleep: {
    name: 'Bleep Audio',
    alt: 'La ventana Bleep Audio con una forma de onda, tres tramos sombreados numerados del 1 al 3 con el tramo 2 seleccionado, una lista de tramos y campos de inicio y fin',
    caption: 'Bleep Audio: tramos marcados en la forma de onda y listados al lado, con tiempos de inicio y fin escritos. Los tramos que se solapan se unen; se guarda una copia.',
  },
  'compress-image': {
    name: 'Compress Images',
    alt: 'La ventana Compress Images con modos Size, Percent y Preset, un tamaño objetivo y una opción de redimensionar',
    caption: 'Compress Images: apunta a un tamaño, un porcentaje o un ajuste. El tamaño que se muestra es una estimación aproximada, no una medición.',
  },
  'compress-video': {
    name: 'Compress Audio and Video',
    alt: 'La ventana Compress Audio and Video para un MP4, con un tamaño objetivo y las tasas de bits que usaría',
    caption: 'Compress Audio and Video: los mismos objetivos para archivos de medios, con las tasas de bits que usaría un tamaño objetivo.',
  },
  meta: {
    name: 'Edit Metadata',
    alt: 'La ventana Edit Metadata con las etiquetas artist, comment y title de un vídeo, botones para borrarlas y la opción Remove all metadata',
    caption: 'Edit Metadata: etiquetas leídas del archivo, editables o borrables, o quitarlas todas. Se guarda una copia.',
  },
  reorder: {
    name: 'Choose Order',
    alt: 'La ventana Choose Order con dos PDF y botones para subir y bajar',
    caption: 'Choose Order: fija el orden de los archivos antes de unir PDF, unir vídeos o unir audio.',
  },
  crop: {
    name: 'Crop Image',
    alt: 'La ventana Crop Image con un recorte 4:3 sobre una imagen y proporciones predefinidas',
    caption: 'Crop Image: un recuadro de recorte con proporciones predefinidas y ancho y alto exactos.',
  },
  edit: {
    name: 'Edit Photo',
    alt: 'La ventana Edit Photo con una imagen y controles de brightness, contrast, saturation, warmth, clarity, sharpen, grain, denoise y dehaze',
    caption: 'Edit Photo: nueve controles y una vista previa sobre una copia reducida; al guardar se escribe una copia a tamaño completo.',
  },
  redact: {
    name: 'Redact Image',
    alt: 'La ventana Redact Image con dos zonas de cobertura sobre una imagen, modos Solid y Blur y opciones de color',
    caption: 'Redact Image: zonas de cobertura sobre la imagen, sólidas o desenfocadas, que se aplican al guardar la copia.',
  },
  frame: {
    name: 'Add Background',
    alt: 'La ventana Add Background con una imagen sobre un fondo azul y controles de padding, corners y shadow',
    caption: 'Add Background: margen, radio de las esquinas y sombra, sobre un fondo de color o desenfocado.',
  },
  collage: {
    name: 'Make a Collage',
    alt: 'La ventana Make a Collage con tres imágenes en un diseño destacado sobre fondo azul, y controles de layout, spacing y corners',
    caption: 'Make a Collage: diseños Grid, Row, Column o Featured, con separación, esquinas y un color de fondo.',
  },
  organize: {
    name: 'Organize PDF',
    alt: 'La ventana Organize PDF con miniaturas de un PDF de tres páginas, una girada y otra duplicada',
    caption: 'Organize PDF: miniaturas de página que puedes girar, duplicar, quitar y reordenar. Aquí la página 2 se giró y se duplicó.',
  },
  annotate: {
    name: 'Annotate Image',
    alt: 'La ventana Annotate Image con una imagen marcada con una flecha, una caja, un número, una llamada, una etiqueta de texto y una lupa, y una fila de herramientas',
    caption: 'Annotate Image: flechas, cajas, números, llamadas, texto y una lupa, mostrados aquí con marcas de ejemplo ya colocadas.',
  },
};
