export const MAX_TITLE = 60;

export interface BuiltPage { path: string; html: string }
export interface LongTitle { path: string; title: string; length: number }

const ENTITIES: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0' };

export function decodeEntities(s: string): string {
  return s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (m, e: string) => {
    if (e[0] === '#') {
      const code = e[1].toLowerCase() === 'x' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
      return Number.isFinite(code) ? String.fromCodePoint(code) : m;
    }
    return ENTITIES[e.toLowerCase()] ?? m;
  });
}

const isDocs = (p: string) => /^\/(es\/)?docs\//.test(p.startsWith('/') ? p : `/${p}`);

/** Non-docs pages whose <title> is longer than MAX_TITLE characters. */
export function longTitles(pages: BuiltPage[]): LongTitle[] {
  const out: LongTitle[] = [];
  for (const { path, html } of pages) {
    if (isDocs(path)) continue;
    const m = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(html);
    if (!m) continue;
    const title = decodeEntities(m[1]).trim();
    if (title.length > MAX_TITLE) out.push({ path, title, length: title.length });
  }
  return out;
}
