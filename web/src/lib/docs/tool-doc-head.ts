import type { HeadTag } from './fallback-head';

export interface ToolDocEntry {
  path: string;
  decision: 'canonical' | 'noindex';
  target: string;
}

const SITE = 'https://ferrocast.app';

/**
 * /docs/files/tools/<slug>/ pages (and their untranslated /es/ fallbacks) that must not compete with /tools/<slug>/.
 * canonical: rel=canonical and og:url point at the product page. noindex: the page keeps its own canonical and gets
 * `noindex,follow`. Either way the hreflang alternates are dropped (a canonicalised or noindex page is not a
 * language alternate) and the page stays reachable with a 200.
 */
export function applyToolDocIndexability(head: HeadTag[], entry: ToolDocEntry): HeadTag[] {
  const out = head
    .filter((t) => !(t.tag === 'link' && t.attrs?.rel === 'alternate' && t.attrs.hreflang))
    .map((t) => ({ ...t, attrs: t.attrs ? { ...t.attrs } : undefined }));
  if (entry.decision === 'canonical') {
    const url = SITE + entry.target;
    for (const t of out) {
      if (t.tag === 'link' && t.attrs?.rel === 'canonical') t.attrs.href = url;
      if (t.tag === 'meta' && t.attrs?.property === 'og:url') t.attrs.content = url;
    }
  } else if (!out.some((t) => t.tag === 'meta' && t.attrs?.name === 'robots')) {
    out.push({ tag: 'meta', attrs: { name: 'robots', content: 'noindex, follow' } });
  }
  return out;
}
