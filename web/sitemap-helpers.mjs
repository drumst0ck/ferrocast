import { readFileSync, readdirSync, statSync } from 'node:fs';
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

/** Released versions from repo CHANGELOG.md (excludes empty Unreleased). */
export function changelogSitemapEntries(site) {
  const md = readFileSync(join(root, '../CHANGELOG.md'), 'utf8');
  const entries = [];
  const re = /^## \[([^\]]+)\](?:\s+-\s+(\d{4}-\d{2}-\d{2}))?/gm;
  let m;
  while ((m = re.exec(md))) {
    const version = m[1].toLowerCase();
    if (version === 'unreleased') continue;
    const releasedOn = m[2] ?? null;
    entries.push({
      url: new URL(`/changelog/${version}/`, site).href,
      lastmod: releasedOn ? new Date(`${releasedOn}T00:00:00Z`) : undefined,
    });
  }
  return entries;
}
