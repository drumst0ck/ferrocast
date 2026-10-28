---
title: "Mosaico automático de ventanas"
description: "Mosaico automático y opcional en Ferrocast: cada ventana nueva tiene su sitio, cuatro disposiciones, modo foco y una disposición por pantalla."
sidebar:
  order: 9
---
El mosaico automático da a cada ventana nueva un sitio en su pantalla, sin arrastrar ni redimensionar nada. Viene **desactivado** porque mueve tus ventanas. Actívalo en Ajustes → Avanzado → Window tiling, o ejecuta **Tile Windows On/Off** desde el lanzador.

## Disposiciones

Elígela en Ajustes, desde el lanzador (**Tiling Layout 1–4**) o con <kbd>⌃</kbd> <kbd>⌥</kbd> <kbd>⌘</kbd> <kbd>1</kbd> a <kbd>4</kbd>. Los atajos cambian la disposición de la pantalla de la ventana enfocada; cada pantalla conserva la suya.

1. **Columnas**: ventanas una junto a otra.
2. **Principal + pila**: la primera a la izquierda y las demás apiladas a la derecha.
3. **Cuadrícula**: filas rellenadas una a una.
4. **Espiral**: cada ventana toma la mitad de lo que queda.

**Space** (8 pt por defecto) es el margen al borde y el hueco entre ventanas.

## Modo foco

<kbd>⌃</kbd> <kbd>⌥</kbd> <kbd>Z</kbd> hace que la ventana enfocada ocupe su pantalla. Púlsalo otra vez y cada ventana vuelve exactamente a su sitio.

## Mover ventanas

<kbd>⌃</kbd> <kbd>⌥</kbd> con una flecha intercambia la ventana enfocada con su vecina en esa dirección. Pasado el borde de la pantalla, va a la siguiente.

## Lo que no se toca

- Los diálogos, hojas, paneles, ventanas flotantes y del sistema, minimizadas o a pantalla completa nunca se colocan.
- Ajustes → Avanzado → **Apps that are not tiled** deja una app fuera.
- Si una app rechaza un tamaño, Ferrocast aprende el mínimo que acepta, mueve las divisiones y flota la ventana en el centro si no cabe. La lista se guarda en tu Mac, por app, sin títulos de ventana.
- Con la pantalla llena, las ventanas sobrantes se apilan en el último hueco en cascada.
- Con **Stage Manager** activo el mosaico se pausa y se reanuda al apagarlo.
- **Pause Tiling** / **Resume Tiling** en el menú de la barra de menús (visible con el mosaico activo) lo detiene sin mover ventanas. El mismo menú tiene un submenú **Tiling Layout** con las cuatro disposiciones.
- **Highlight border** dibuja un borde de 3 pt, del color de **Border color**, alrededor de la ventana enfocada que está en el mosaico. Se oculta con el mosaico en pausa o apagado, con Stage Manager, en ventanas flotantes y un instante al entrar o salir del modo foco.
- **Quick actions above windows** (desactivado por defecto) muestra una barra pequeña sobre una ventana cuando el puntero se queda en su borde superior (en los 28 pt superiores de la ventana o sobre la propia barra). La barra va encima de la ventana si hay sitio; si no, es una pastilla en la esquina superior derecha, bajo la barra de título, para no tapar pestañas ni botones. No aparece con un botón del ratón pulsado, y el borde resaltado también se oculta al arrastrar o redimensionar. Sus botones son los mismos comandos del lanzador y los atajos: **modo foco**, **flotar o volver al mosaico**, **mover a la siguiente pantalla**, **intercambiar con el hueco principal** y **dejar de colocar esta app**.
- El modo foco no hace nada con Stage Manager activo. Los atajos del mosaico nunca sustituyen uno que ya tengas; si chocan, Ferrocast te avisa y deja ese comando sin atajo.

:::caution[Límites]
Necesita el permiso de Accesibilidad.
:::
