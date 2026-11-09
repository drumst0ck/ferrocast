export type Lang = 'en' | 'es';

/** Accessible names for the shared site chrome, in the page language. */
export const siteLabels = {
  en: { mainNav: 'Main', menu: 'Menu', links: 'Links', docs: 'Docs', home: 'Home', files: 'Files', changelog: 'Changelog' },
  es: { mainNav: 'Principal', menu: 'Menú', links: 'Enlaces', docs: 'Documentación', home: 'Inicio', files: 'Archivos', changelog: 'Cambios' },
} as const;

/** Home URL for a language (docs header logo and links must stay in the visitor's language). */
export function homeHref(lang: Lang): string {
  return lang === 'es' ? '/es/' : '/';
}
