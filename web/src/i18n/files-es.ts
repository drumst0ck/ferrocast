import { counts } from '../data/status';
import { languageFaqEs } from '../lib/seo/interface-language';

export function filesEsMeta() {
  const n = counts();
  const title = 'Convertir y herramientas de archivo en Mac · Ferrocast';
  const description = `Convierte entre ${n.pairs} pares de formatos y usa herramientas de imagen, vídeo, PDF y audio desde una rueda. Todo ocurre en tu Mac y no se sube nada.`;
  if (description.length < 140 || description.length > 155) {
    throw new Error(`es files description is ${description.length} characters`);
  }
  return { title, description, n };
}

export const filesEs = {
  h1: 'Arrastra un archivo. Pulsa una tecla. Suéltalo.',
  lead: 'Mantén un archivo, pulsa Mayús para conversiones, Comando para herramientas o Control para acciones. La rueda sigue abierta hasta que lo sueltas. Todo ocurre en tu Mac; no se sube nada. Puedes usarlo con la interfaz en español.',
  keysTitle: 'Tres teclas, tres ruedas',
  keysLead: 'Empieza a arrastrar un archivo y pulsa una tecla. Pulsa otra para cambiar de rueda; la rueda sigue abierta hasta que sueltas o cancelas.',
  wheelTitle: 'Prueba la rueda',
  wheelLead: 'Una imagen de cada rueda, hecha con las listas que usa la app. Elige una tecla y muévete con las flechas.',
  conversionsTitle: 'Todas las conversiones, para buscar',
  conversionsLead: 'Mira en qué puede convertirse un formato. Los 275 pares están listados, agrupados por familia.',
  conversionsNote: 'Extraer RAR necesita un unrar que funcione, y .pages se escribe en el formato iWork ’09. ffmpeg y LibreOffice se usan si están instalados; las herramientas de macOS van primero.',
  toolsLead: 'Ventanas y ajustes para el archivo que tienes. Escribe una orden corta en la vista Convert para saltarte la ventana.',
  actionsTitle: 'Siete acciones de archivo',
  actionsLead: 'Mantén Control mientras arrastras cualquier archivo o carpeta.',
  actionsNote: 'Enter sobre una selección del Finder es un ajuste opcional en Ajustes > General (desactivado por defecto, necesita Accesibilidad); la orden Convert Finder Selection también funciona.',
  galleryTitle: 'Las ventanas, en imágenes',
  galleryLead: 'Qué aspecto tienen las ventanas de las herramientas y las ruedas.',
  galleryNote: 'Generadas por el modo de capturas de la propia app, sobre archivos de ejemplo. Son imágenes de las ventanas, no grabaciones de uso.',
  faqTitle: 'Preguntas sobre las conversiones',
  backHome: 'Volver a la página de inicio de Ferrocast',
  changelog: 'Leer el registro de cambios',
  toolsIndex: 'Índice de herramientas',
  popular: 'Conversiones más buscadas',
  docs: 'Documentación',
};

export const filesEsActions: Record<string, [string, string]> = {
  airDrop: ['AirDrop', 'Envía los archivos seleccionados con el selector de compartir del sistema.'],
  float: ['Estante flotante', 'Deja los archivos en un estante flotante pequeño que permanece encima y se ancla a un borde de la pantalla. Se pueden soltar archivos encima.'],
  copy: ['Copiar', 'Pone los archivos en el portapapeles, listos para pegar en el Finder.'],
  move: ['Mover a…', 'Elige una carpeta de destino y mueve los archivos allí. Un archivo que ya existe nunca se sobrescribe.'],
  copyPath: ['Copiar ruta', 'Copia la ruta de cada elemento seleccionado, una por línea.'],
  openTerminal: ['Terminal / iTerm2', 'Abre una carpeta, o la carpeta que contiene un archivo, en Terminal o iTerm2.'],
  zip: ['Zip', 'Comprime cada archivo o carpeta seleccionado en su propio ZIP junto a él. Un ZIP que ya existe nunca se sobrescribe.'],
};

export function filesEsFaq(n: { inputs: number; pairs: number }): [string, string][] {
  return [
    ['¿Las conversiones son realmente locales?', 'Sí. No se sube nada. Las conversiones se ejecutan en tu Mac y usan primero las herramientas que vienen con macOS. ffmpeg, LibreOffice, cwebp, unrar y Ghostscript son opcionales y solo se usan si están instalados; las pastillas de esta página indican qué pares necesitan una.'],
    ['¿Qué puede convertir?', `Hay ${n.inputs} formatos de entrada y ${n.pairs} pares de conversión entre imágenes, audio, vídeo, PDF, documentos, presentaciones, hojas de cálculo, subtítulos y archivos comprimidos. Usa el explorador de arriba para buscarlos.`],
    ['¿Sobrescribe mis archivos?', 'No. El resultado se escribe junto al original, con el mismo nombre y la extensión nueva. Si ese nombre está ocupado, añade 2, 3 y así sucesivamente.'],
    ['¿Puedo pulsar Intro sobre un archivo en el Finder?', 'Sí, como ajuste opcional. En Ajustes > General, activa «Enter on Finder selection» (desactivado por defecto, necesita Accesibilidad) y pulsar Retorno sobre archivos seleccionados en el Finder abre la rueda Convertir en lugar de renombrar. La orden Convert Finder Selection también funciona, y Opción-Intro sobre un resultado de archivo en el lanzador hace lo mismo.'],
    languageFaqEs,
  ];
}
