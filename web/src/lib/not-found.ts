export type Lang = 'en' | 'es';

export const notFoundCopy = {
  en: { title: 'Page not found · Ferrocast', description: 'That page does not exist.', heading: 'No results', body: 'That page does not exist. Try the home page instead.', cta: 'Back to Ferrocast' },
  es: { title: 'Página no encontrada · Ferrocast', description: 'Esa página no existe.', heading: 'Sin resultados', body: 'Esa página no existe. Prueba con la página de inicio.', cta: 'Volver a Ferrocast' },
} as const;

/** Spanish 404 for anything under /es/, English otherwise. */
export function notFoundLang(pathname: string): Lang {
  return pathname === '/es' || pathname.startsWith('/es/') ? 'es' : 'en';
}
