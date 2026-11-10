import type { Lang } from '../site-labels';

export interface CookiePolicyCopy {
  path: string;
  title: string;
  description: string;
  crumbs: [string, string][];
  h1: string;
  lead: string;
  storedH: string;
  cols: [string, string, string, string, string];
  rows: [string, string, string, string, string][];
  rowsNote: string;
  withdrawH: string;
  withdraw: string;
  changeButton: string;
  moreH: string;
  more: string;
}

export const policyCopy: Record<Lang, CookiePolicyCopy> = {
  en: {
    path: '/cookies/',
    title: 'Cookie policy | Ferrocast',
    description: 'What ferrocast.app stores in your browser: one local setting, plus Google Analytics cookies only if you accept. How to change or withdraw your choice.',
    crumbs: [['Home', '/'], ['Cookie policy', '/cookies/']],
    h1: 'Cookie policy',
    lead: 'ferrocast.app counts visits with Google Analytics, loaded through Google Tag Manager. Nothing from Google is loaded and no analytics cookie is set until you click Accept. If you click Reject, or ignore the banner, nothing is loaded.',
    storedH: 'What is stored in your browser',
    cols: ['Name', 'Purpose', 'Duration', 'Provider', 'When'],
    rows: [
      ['ferrocast-consent (local storage)', 'Remembers your cookie choice and the policy version you answered.', 'Until you clear it or the policy changes', 'Ferrocast (first party)', 'Always, once you choose. Not a cookie and never sent to a server.'],
      ['sl-sidebar-state (session storage)', 'Remembers the open and closed sections and scroll position of the documentation sidebar.', 'Until you close the tab', 'Ferrocast (first party)', 'Only on the documentation pages. Strictly necessary, never sent to a server.'],
      ['starlight-theme (local storage)', 'Remembers light or dark mode on the documentation pages.', 'Until you clear it', 'Ferrocast (first party)', 'Only if you pick a theme in the docs.'],
      ['_ga', 'Tells visitors apart to count visits and pages.', 'Up to 2 years (browsers may cap it at about 13 months)', 'Google', 'Only after you accept.'],
      ['_ga_<container id>', 'Keeps the state of the current visit.', 'Up to 2 years (browsers may cap it at about 13 months)', 'Google', 'Only after you accept.'],
    ],
    rowsNote: 'Google Consent Mode v2 starts with every Google storage type denied. Accepting grants analytics storage only. Advertising storage, user data and personalization stay denied.',
    withdrawH: 'Change or withdraw your choice',
    withdraw: 'Use the Cookie settings link in the footer of any page, or the button below, to see the banner again and choose Accept or Reject. Rejecting stops Google Tag Manager from loading on the next page and removes the _ga cookies this site can reach. Your browser settings can also delete cookies and local storage. We ask again when this policy changes.',
    changeButton: 'Cookie settings',
    moreH: 'More information',
    more: 'Google describes how it uses data in its Privacy & Terms for Analytics. The Ferrocast app itself does not use these cookies.',
  },
  es: {
    path: '/es/cookies/',
    title: 'Política de cookies | Ferrocast',
    description: 'Qué guarda ferrocast.app en tu navegador: un ajuste local y, solo si aceptas, las cookies de Google Analytics. Cómo cambiar o retirar tu elección.',
    crumbs: [['Inicio', '/es/'], ['Política de cookies', '/es/cookies/']],
    h1: 'Política de cookies',
    lead: 'ferrocast.app cuenta las visitas con Google Analytics, cargado mediante Google Tag Manager. No se carga nada de Google ni se guarda ninguna cookie de analítica hasta que pulsas Aceptar. Si pulsas Rechazar, o ignoras el aviso, no se carga nada.',
    storedH: 'Qué se guarda en tu navegador',
    cols: ['Nombre', 'Finalidad', 'Duración', 'Proveedor', 'Cuándo'],
    rows: [
      ['ferrocast-consent (almacenamiento local)', 'Recuerda tu elección sobre cookies y la versión de la política que respondiste.', 'Hasta que lo borres o cambie la política', 'Ferrocast (propia)', 'Siempre, una vez que eliges. No es una cookie y nunca se envía a un servidor.'],
      ['sl-sidebar-state (almacenamiento de sesión)', 'Recuerda las secciones abiertas y cerradas y la posición de desplazamiento del menú lateral de la documentación.', 'Hasta que cierres la pestaña', 'Ferrocast (propia)', 'Solo en las páginas de documentación. Estrictamente necesaria, nunca se envía a un servidor.'],
      ['starlight-theme (almacenamiento local)', 'Recuerda el modo claro u oscuro en las páginas de documentación.', 'Hasta que lo borres', 'Ferrocast (propia)', 'Solo si eliges un tema en la documentación.'],
      ['_ga', 'Distingue a los visitantes para contar visitas y páginas.', 'Hasta 2 años (los navegadores pueden limitarla a unos 13 meses)', 'Google', 'Solo después de aceptar.'],
      ['_ga_<id del contenedor>', 'Mantiene el estado de la visita actual.', 'Hasta 2 años (los navegadores pueden limitarla a unos 13 meses)', 'Google', 'Solo después de aceptar.'],
    ],
    rowsNote: 'Google Consent Mode v2 empieza con todos los tipos de almacenamiento de Google denegados. Aceptar concede solo el almacenamiento de analítica. El almacenamiento publicitario, los datos de usuario y la personalización siguen denegados.',
    withdrawH: 'Cambiar o retirar tu elección',
    withdraw: 'Usa el enlace Configuración de cookies del pie de cualquier página, o el botón de abajo, para ver el aviso otra vez y elegir Aceptar o Rechazar. Al rechazar, Google Tag Manager deja de cargarse en la página siguiente y se borran las cookies _ga que este sitio puede alcanzar. También puedes borrar cookies y almacenamiento local desde los ajustes del navegador. Volvemos a preguntar cuando cambia esta política.',
    changeButton: 'Configuración de cookies',
    moreH: 'Más información',
    more: 'Google explica cómo usa los datos en su documento de Privacidad y Términos de Analytics. La aplicación Ferrocast no usa estas cookies.',
  },
};
