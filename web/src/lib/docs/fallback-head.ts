/** One Starlight `<head>` entry. Attrs are copied before they are rewritten. */
export interface HeadTag {
  tag: string;
  content?: string;
  attrs?: Record<string, string | boolean | undefined>;
}

/**
 * Untranslated `/es/docs/` routes are Starlight fallbacks: the body stays the English source.
 * Point canonical, Open Graph URL, and hreflang at the English page, and keep `noindex`,
 * so the fallback URL is not a second Spanish document.
 * Translated pages are unchanged apart from the existing `x-default` link.
 */
export function rewriteStarlightHead(
  head: HeadTag[],
  opts: { isFallback: boolean; esAvailable: boolean; enCanonical?: string },
): HeadTag[] {
  const { isFallback, esAvailable, enCanonical } = opts;
  const filtered = head
    .filter((item) => {
      if (item.tag !== 'link' || item.attrs?.rel !== 'alternate') return true;
      const hl = item.attrs.hreflang;
      if (hl === 'x-default') return false;
      if (isFallback) return false;
      if (hl === 'es' && !esAvailable) return false;
      return true;
    })
    .map((item) => ({ ...item, attrs: item.attrs ? { ...item.attrs } : undefined }));

  if (isFallback) {
    if (enCanonical) {
      for (const item of filtered) {
        const attrs = item.attrs;
        if (!attrs) continue;
        if (item.tag === 'link' && attrs.rel === 'canonical') attrs.href = enCanonical;
        if (item.tag === 'meta' && attrs.property === 'og:url') attrs.content = enCanonical;
        if (item.tag === 'meta' && attrs.property === 'og:locale') attrs.content = 'en';
      }
      filtered.push({ tag: 'link', attrs: { rel: 'alternate', hreflang: 'en', href: enCanonical } });
      filtered.push({ tag: 'link', attrs: { rel: 'alternate', hreflang: 'x-default', href: enCanonical } });
    }
    filtered.push({ tag: 'meta', attrs: { name: 'robots', content: 'noindex, follow' } });
    return filtered;
  }

  if (enCanonical) {
    filtered.push({ tag: 'link', attrs: { rel: 'alternate', hreflang: 'x-default', href: enCanonical } });
  }
  return filtered;
}
