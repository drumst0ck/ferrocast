/*
 * Per-page og:image. Every non-docs page that has a canonical URL (except the two homes, which keep the brand image
 * og-welcome.jpg) points at /og/<slug>.png, a 1200×630 file written into dist by integrations/og-images.mjs after the
 * build. The image text is the page title without the " | Ferrocast" suffix.
 */

export const OG_WIDTH = 1200;
export const OG_HEIGHT = 630;
const SUFFIX = / \| Ferrocast$/;

function pathnameOf(canonical: string): string {
  return new URL(canonical, 'https://ferrocast.app').pathname;
}

/** `prerendered: false` (server-rendered routes such as /changelog/) keeps the brand image: the build only writes PNGs
 * for pages it prerenders, so those routes would otherwise point at a missing file. */
export function hasOwnOgImage(canonical?: string, prerendered = true): boolean {
  if (!canonical || !prerendered) return false;
  const p = pathnameOf(canonical);
  return p !== '/' && p !== '/es/';
}

/** "/es/a/b/" -> "/og/es--a--b.png" (one flat, unique file per page). */
export function ogImagePath(canonical: string): string {
  const slug = pathnameOf(canonical).split('/').filter(Boolean).join('--');
  return `/og/${slug}.png`;
}

export function ogText(title: string): string {
  return title.replace(SUFFIX, '').trim() || title;
}

export function ogAlt(title: string): string {
  return `Ferrocast: ${ogText(title)}`;
}
