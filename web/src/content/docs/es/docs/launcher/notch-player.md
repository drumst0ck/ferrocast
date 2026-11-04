---
title: "Reproductor en el notch"
description: "Muestra en el notch lo que suena en tu Mac (navegador, Spotify, Música, Podcasts) como un vinilo que gira, con título, barra de tiempo y controles."
sidebar:
  order: 21
---
El reproductor del notch muestra lo que suena en tu Mac en una pequeña píldora negra: cualquier app que informe a «Ahora suena» de macOS, como una pestaña del navegador (YouTube, Spotify Web), Spotify, Música, Podcasts o VLC. En un MacBook con notch, la píldora prolonga el notch con un ala a cada lado; en cualquier otro Mac es una cápsula arriba, en el centro de la pantalla. Viene **desactivado**.

Actívalo en Ajustes → General → Reproductor del notch → **Reproduciendo ahora en el notch**. Se activa y se desactiva al instante, sin reiniciar, y el ajuste se guarda con el resto.

## Qué ves

- **Plegado**: la carátula como un pequeño disco que gira a la izquierda y un ecualizador a la derecha mientras suena una canción. Las barras quedan bajas en pausa, y la píldora da un pequeño pulso cuando empieza una pista nueva.
- **Al pasar el puntero**: la píldora se abre con una animación elástica, como de líquido, hasta una tarjeta: un vinilo grande con la carátula (la carátula llena el disco sin deformarse; una imagen ancha o alta se recorta alrededor del centro), un anillo alrededor que se va llenando con la canción, un título grande en negrita, el artista, el nombre y el icono de la app que suena, **anterior**, **pausa/reproducir** (el icono se transforma) y **siguiente**, y el tiempo transcurrido y restante. Un resplandor difuminado de la carátula tiñe la tarjeta y una luz suave de su color principal se derrama por debajo. El disco gira mientras suena, se detiene poco a poco al pausar y da un giro rápido al empezar una pista nueva. Al retirar el puntero, la tarjeta se cierra tras una breve pausa. Con **Reducir movimiento** activado en Ajustes del Sistema → Accesibilidad → Pantalla, se abre y se cierra sin animación, el disco no gira, el resplandor no se desplaza y las barras no se mueven.
- **Adelantar o retroceder**: haz clic en cualquier punto de la barra de progreso para saltar ahí, o pulsa y arrastra para recorrerla. La barra muestra un tirador y, mientras arrastras, una burbuja con el tiempo; la canción solo salta al soltar. La barra es fácil de acertar (unos 16 pt de alto) y el puntero se vuelve una mano sobre ella. Una pista sin duración, como una emisión en directo, no se puede recorrer.
- **Abrir la app**: haz clic en el título o el artista (se resaltan y muestran una mano) para traer al frente la app que suena; si es una pestaña del navegador, el navegador.
- Los colores salen de la carátula (su color principal tiñe el anillo, la barra de progreso y el resplandor); una pista sin carátula usa el acento de Ferrocast.

Se oculta sola cuando:

- no suena nada: ninguna app informa de contenido o una canción lleva en pausa unos 45 segundos (vuelve al pulsar reproducir; nunca se oculta mientras el puntero está encima);
- una app está a pantalla completa en la pantalla principal.

## Cómo funciona y permisos

Ferrocast lee «Ahora suena» de macOS con **mediaremote-adapter**, un pequeño ayudante de código abierto (BSD de 3 cláusulas, consulta los [avisos de terceros](https://github.com/drumst0ck/ferrocast/blob/master/THIRD_PARTY_NOTICES.md)) que viene dentro de la app. Se ejecuta con el `/usr/bin/perl` del propio macOS, porque desde macOS 15.4 solo las herramientas de Apple pueden leer «Ahora suena» directamente.

- **No pide ningún permiso.** No usa Automatización, Accesibilidad ni Grabación de pantalla, no accede a internet y no guarda la carátula en disco: sale de la memoria.
- Solo se ejecuta mientras el ajuste está activado. Al desactivarlo, o al cerrar Ferrocast, el ayudante se detiene al instante.
- Anterior, pausa/reproducir y siguiente se envían a la app que macOS considera el reproductor actual.

## Límites

- **Puede dejar de funcionar tras una actualización de macOS.** El ayudante usa una parte privada de macOS que Apple puede cambiar. Si falla de forma repetida, Ferrocast deja de reintentar, muestra una vez «Now playing is not available on this version of macOS» (en tu idioma) y mantiene la píldora oculta hasta que desactives y actives el ajuste, o actualices Ferrocast.
- Solo se muestra lo que la app que suena cuenta a macOS. Una app que no informa a «Ahora suena» no aparece, y algunas no envían carátula ni duración (sin duración la barra de progreso queda vacía).
- Las carátulas de más de 4 MB o de más de 2048 píxeles por lado no se muestran (aparece el marcador). Al pausar se mantiene la carátula de la misma canción.
- Si ocultas la barra de menús y aparece al llevar el puntero al borde superior, la píldora se queda por encima. Solo se oculta con una app a pantalla completa de verdad.
- El ecualizador es decorativo: macOS no deja que una app lea el nivel de audio de otra sin el permiso de Grabación de pantalla, que esta función no pide.
- Un cambio hecho en el reproductor se ve casi al instante (las actualizaciones se agrupan durante una fracción de segundo).
- Con notch, la píldora se ancla a la pantalla principal (la de la barra de menús). El panel le pide a macOS que no aparezca en capturas ni grabaciones de pantalla; algunas herramientas de captura pueden mostrarlo igualmente.
- En un Mac sin notch la cápsula queda sobre el centro de la barra de menús y puede tapar un elemento de ella.

## Relacionado

- [Ajustes](/es/docs/launcher/settings/)
