import { describe, expect, it } from 'vitest';
import { rewriteStarlightHead, type HeadTag } from './fallback-head';

const EN = 'https://ferrocast.app/docs/launcher/snippets/';
const ES = 'https://ferrocast.app/es/docs/launcher/snippets/';

function sample(): HeadTag[] {
  return [
    { tag: 'link', attrs: { rel: 'canonical', href: ES } },
    { tag: 'meta', attrs: { property: 'og:url', content: ES } },
    { tag: 'meta', attrs: { property: 'og:locale', content: 'es' } },
    { tag: 'link', attrs: { rel: 'alternate', hreflang: 'en', href: EN } },
    { tag: 'link', attrs: { rel: 'alternate', hreflang: 'es', href: ES } },
    { tag: 'meta', attrs: { name: 'description', content: 'English body' } },
  ];
}

describe('rewriteStarlightHead', () => {
  it('points every untranslated /es/docs/ fallback at the English page', () => {
    const out = rewriteStarlightHead(sample(), { isFallback: true, esAvailable: false, enCanonical: EN });
    const canonical = out.find((t) => t.tag === 'link' && t.attrs?.rel === 'canonical');
    const ogUrl = out.find((t) => t.attrs?.property === 'og:url');
    const ogLocale = out.find((t) => t.attrs?.property === 'og:locale');
    const alternates = out.filter((t) => t.tag === 'link' && t.attrs?.rel === 'alternate');
    const robots = out.find((t) => t.attrs?.name === 'robots');
    expect(canonical?.attrs?.href).toBe(EN);
    expect(ogUrl?.attrs?.content).toBe(EN);
    expect(ogLocale?.attrs?.content).toBe('en');
    expect(alternates.map((t) => [t.attrs?.hreflang, t.attrs?.href])).toEqual([
      ['en', EN],
      ['x-default', EN],
    ]);
    expect(robots?.attrs?.content).toBe('noindex, follow');
    expect(out.some((t) => t.attrs?.hreflang === 'es')).toBe(false);
  });

  it('leaves a real Spanish translation canonical on itself', () => {
    const notes = 'https://ferrocast.app/es/docs/launcher/notes/';
    const en = 'https://ferrocast.app/docs/launcher/notes/';
    const head: HeadTag[] = [
      { tag: 'link', attrs: { rel: 'canonical', href: notes } },
      { tag: 'meta', attrs: { property: 'og:url', content: notes } },
      { tag: 'meta', attrs: { property: 'og:locale', content: 'es' } },
      { tag: 'link', attrs: { rel: 'alternate', hreflang: 'en', href: en } },
      { tag: 'link', attrs: { rel: 'alternate', hreflang: 'es', href: notes } },
    ];
    const out = rewriteStarlightHead(head, { isFallback: false, esAvailable: true, enCanonical: en });
    expect(out.find((t) => t.attrs?.rel === 'canonical')?.attrs?.href).toBe(notes);
    expect(out.find((t) => t.attrs?.property === 'og:locale')?.attrs?.content).toBe('es');
    expect(out.find((t) => t.attrs?.name === 'robots')).toBeUndefined();
    const langs = out.filter((t) => t.attrs?.rel === 'alternate').map((t) => t.attrs?.hreflang);
    expect(langs).toEqual(['en', 'es', 'x-default']);
    expect(out.find((t) => t.attrs?.hreflang === 'x-default')?.attrs?.href).toBe(en);
  });
});
