import { readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ES_DOCS_ROOT = join(process.cwd(), 'src/content/docs/es/docs');

/** Slugs relative to `docs/` (empty string = docs home). Derived from ES source files only. */
function collectTranslatedSlugs(dir: string, base = dir): Set<string> {
  const slugs = new Set<string>();
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      for (const s of collectTranslatedSlugs(path, base)) slugs.add(s);
      continue;
    }
    if (!/\.(md|mdx)$/i.test(name)) continue;
    let rel = relative(base, path).replace(/\\/g, '/').replace(/\.(md|mdx)$/i, '');
    if (rel.endsWith('/index')) rel = rel.slice(0, -'/index'.length);
    else if (rel === 'index') rel = '';
    slugs.add(rel);
  }
  return slugs;
}

export const translatedEsDocSlugs: Set<string> = collectTranslatedSlugs(ES_DOCS_ROOT);

/** Starlight `entry.slug` or pathname segment after `/docs/`. */
export function docsRelativeSlug(slugOrPath: string): string {
  const s = slugOrPath.replace(/^\//, '').replace(/\/$/, '');
  const withoutLocale = s.replace(/^es\/docs\/?/, '').replace(/^docs\/?/, '');
  return withoutLocale === 'index' ? '' : withoutLocale;
}

export function hasEsTranslation(slugOrPath: string): boolean {
  return translatedEsDocSlugs.has(docsRelativeSlug(slugOrPath));
}

export function isEsDocsFallbackPath(pathname: string): boolean {
  const path = pathname.replace(/\/$/, '');
  if (!path.includes('/es/docs')) return false;
  const idx = path.indexOf('/es/docs');
  const rest = path.slice(idx + '/es/docs'.length).replace(/^\//, '');
  const slug = rest === 'index' ? '' : rest;
  return !translatedEsDocSlugs.has(slug);
}
