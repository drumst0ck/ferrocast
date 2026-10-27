import { readdirSync, statSync } from 'node:fs';
import { join, relative, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const ES_DOCS_ROOT = join(root, 'src/content/docs/es/docs');

function collectTranslatedSlugs(dir, base = dir) {
  const slugs = new Set();
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

const translatedEsDocSlugs = collectTranslatedSlugs(ES_DOCS_ROOT);

export function isEsDocsFallbackUrl(url) {
  const pathname = new URL(url).pathname.replace(/\/$/, '');
  if (!pathname.includes('/es/docs')) return false;
  const idx = pathname.indexOf('/es/docs');
  const rest = pathname.slice(idx + '/es/docs'.length).replace(/^\//, '');
  const slug = rest === 'index' ? '' : rest;
  return !translatedEsDocSlugs.has(slug);
}
