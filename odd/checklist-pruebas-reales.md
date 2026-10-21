# Pruebas reales H1–H16 (las haces tú, con tu ratón, teclado y pantalla)

Fuente: `odd/plans/tangerine-parity-plan.md`, líneas 566–581. Aquí cada prueba tiene los pasos, lo
esperado y su estado. **Un ✅ solo significa que tú lo viste funcionar.** Lo que probé yo con tests o
capturas no cuenta como prueba real.

Cómo reportar: una línea por prueba, por ejemplo `H5 bien` o `H8 el arrastre salta una fila`. Si algo falla,
copia las últimas líneas de `~/Library/Application Support/ferrocast/ferrocast.log` (registra la rueda,
las elecciones y los errores de conversión).

Paquete a probar: `dist/Ferrocast.app` reconstruido con `scripts/bundle.sh` (ábrelo con `open`).

| Prueba | Estado |
|---|---|
| H1 rueda | ✅ parcial: ⌘ abre la ventana de la herramienta, Shift enseña conversiones, etiquetas visibles |
| H2 teclas en la rueda | sin probar |
| H3 Enter sobre Finder | no construido todavía |
| H4 estantería, AirDrop, Terminal, selección de Finder | sin probar |
| H5 sliders y estimación de compresión | sin probar |
| H6 arrastre en recorte, redacción, anotar; selector de sello | sin probar |
| H7 organizador de PDF grande | sin probar |
| H8 arrastrar para reordenar | sin probar |
| H9 audio de 30 minutos, recorte, pitido, canales | sin probar |
| H10 RAR real | sin probar |
| H11 `.pages` en Pages | sin probar |
| H12 desenfoque nativo | no construido todavía |
| H13 aspecto de la rueda (claro/oscuro, 6/8/9 segmentos, Reducir movimiento, Retina) | sin probar; las animaciones están en construcción |
| H14 vídeo girado y de frame variable | sin probar |
| H15 metadatos en un JPEG de cámara y un audio con capítulos | sin probar |
| H16 la web | parte automática hecha por mí: con solo teclado se llega al filtro (11 Tab) y al escribir «pdf» el contador pasa de 275 a 32 pares, con foco visible (contorno de 2 px). Falta lo tuyo: móvil real, Reducir movimiento en tu Mac y que el texto coincida con el README |

## Pasos

**H1 — Rueda.** Arrastra un archivo desde Finder (sin soltarlo). Toca Shift, luego ⌘, luego Control: la rueda se
abre bajo el puntero, cambia de tipo y no se cierra al soltar la tecla. Suelta sobre un segmento: se ejecuta. Suelta
fuera del anillo: se cancela. Control debe mostrar siete acciones, con Zip la última.

**H2 — Teclas.** Con la rueda abierta: flechas mueven la selección, Intro ejecuta, Esc cancela. Sobre un resultado
del buscador, ⌥↵ debe abrir la rueda.

**H4 — Estantería y sistema.** «Float»: el panel queda siempre encima, se pega a un borde y admite arrastrar
dentro y fuera. AirDrop abre el selector. Terminal e iTerm abren la carpeta. El comando de la selección de
Finder pide permiso de Automatización la primera vez.

**H5 — Sliders.** Compress, Edit Photo, Background y Collage: los sliders responden sin tirones. En Compress,
con una foto de unos 12 megapíxeles, la estimación se parece al tamaño final.

**H6 — Arrastre.** En recorte, redacción y anotar: mover y redimensionar con el ratón, el campo de texto, el
arrastre de globos y el selector de imagen para el sello.

**H7 — Organizador.** Con un PDF grande: miniaturas, teclas (R girar, D duplicar, ⌫ quitar, ⌘←/→ mover). Estas
teclas ya no se anuncian en el pie, pero funcionan.

**H8 — Reordenar.** Arrastrar filas en Choose Order, miniaturas en el collage y páginas en el organizador. Fíjate
en que el orden final coincide con lo que arrastraste y en qué pasa con listas más largas que la ventana (no hay
desplazamiento automático).

**H9 — Audio.** Un archivo de 30 minutos: la forma de onda no debe congelar la app. Recorte, pitido y canales
deben sonar bien (escucha el resultado).

**H10 — RAR.** Un `.rar` real (hecho con una herramienta oficial o arm64) se extrae.

**H11 — Pages.** Un `.pages` generado por Ferrocast se abre en Pages.app.

**H13 — Rueda.** Oscuro y claro, con 6, 8 y 9 segmentos; brillo al pasar; Reducir movimiento activado en Ajustes del
Sistema; pantalla Retina.

**H14 — Vídeo.** Un vídeo de móvil girado por recorte y redacción; uno de frame variable por capturas.

**H15 — Metadatos.** Edita los de un JPEG de cámara y de un audio con capítulos y comprueba el resultado en otra app.

**H16 — Web.** `cd web && npm run dev` (o `npm run build && npm run preview`). Con solo teclado: el filtro de la
matriz de conversiones, el tamaño móvil, Reducir movimiento y que el texto coincida con el estado del README.

## Lo que ya sé que no está cubierto

- Cualquier prueba con ratón, teclado o pantalla reales de las ventanas nuevas (tarjetas, pie con teclas, selectores).
- El tema claro en tu pantalla.
- La rueda de conversiones de un PDF (nueve salidas con nombres largos).
- Que la notificación de «Metadata · 1 file written» aparece de verdad.
