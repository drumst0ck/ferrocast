import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

/** Source-level anchor check: every `/docs/...#anchor` (or `/es/docs/...#anchor`) link and same-page `#anchor` must hit a real heading. */
const root = join(process.cwd(), 'src/content/docs');

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) return n === '_generated' ? [] : files(p);
    return /\.mdx?$/.test(n) ? [p] : [];
  });
}

const stripInline = (t: string) =>
  t.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').replace(/<[^>]+>/g, '').replace(/[`*_~]/g, (c) => (c === '_' ? c : '')).trim();

const slug = (t: string) =>
  t.toLowerCase().replace(/[^\p{L}\p{M}\p{N}\p{Pc} -]/gu, '').replace(/ /g, '-');

function headingSlugs(path: string): Set<string> {
  const out = new Set<string>();
  const counts = new Map<string, number>();
  let fence = false;
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) fence = !fence;
    if (fence) continue;
    const m = line.match(/^#{1,6}\s+(.*?)\s*#*\s*$/);
    if (!m) continue;
    let s = slug(stripInline(m[1]));
    const n = counts.get(s) ?? 0;
    counts.set(s, n + 1);
    if (n) s = `${s}-${n}`;
    out.add(s);
  }
  return out;
}

function pageFile(urlPath: string): string | null {
  const rel = urlPath.replace(/^\//, '').replace(/\/$/, '');
  const tries = [join(root, rel, 'index.mdx'), join(root, rel, 'index.md'), join(root, `${rel}.mdx`), join(root, `${rel}.md`)];
  let found = tries.find(existsSync) ?? null;
  // Untranslated /es/docs pages are Starlight fallbacks that render the English body.
  if (!found && rel.startsWith('es/')) found = pageFile('/' + rel.slice(3));
  return found;
}

describe('docs anchors', () => {
  it('every internal docs link with a #fragment points at an existing heading', () => {
    const broken: string[] = [];
    for (const file of files(root)) {
      const text = readFileSync(file, 'utf8');
      const here = relative(root, file);
      for (const m of text.matchAll(/\]\((\/[^)\s#]*)?#([^)\s]+)\)/g)) {
        const [, path, frag] = m;
        const target = path ? (/^\/(?:es\/)?docs\//.test(path) || /^\/(?:es\/)?docs$/.test(path) ? pageFile(path) : 'skip') : file;
        if (target === 'skip') continue;
        if (!target) {
          broken.push(`${here}: ${path}#${frag} (page not found)`);
          continue;
        }
        if (!headingSlugs(target).has(decodeURIComponent(frag))) broken.push(`${here}: ${path ?? ''}#${frag}`);
      }
    }
    expect(broken).toEqual([]);
  });

  it('flags the ES Settings link that used the English #updates anchor', () => {
    const settings = readFileSync(join(root, 'es/docs/launcher/settings.md'), 'utf8');
    expect(settings).not.toContain('install/#updates');
    expect(headingSlugs(join(root, 'es/docs/getting-started/install.md')).has('actualizaciones')).toBe(true);
  });
});
