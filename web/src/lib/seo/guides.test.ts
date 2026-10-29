import { existsSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { allGuidePaths, featuredGuides, guideBlocks, relatedGuides, type GuideLang } from './guides';
import { page as organizePdf } from './organize-pdf';

/** Pages generated from a dynamic route; the slug is checked against the pages that use it. */
const generated = new Set([organizePdf.path]);
const exists = (p: string) => {
  const rel = p.replace(/^\/|\/$/g, '');
  return generated.has(p) || existsSync(`src/pages/${rel}.astro`) || existsSync(`src/pages/${rel}/index.astro`);
};

describe('guide registry', () => {
  it('only lists pages that exist, once each', () => {
    const paths = allGuidePaths();
    expect(paths.filter((p) => !exists(p))).toEqual([]);
    expect(new Set(paths).size).toBe(paths.length);
  });
  it('keeps each language to its own pages', () => {
    for (const lang of ['en', 'es'] as GuideLang[]) {
      for (const b of guideBlocks(lang)) for (const x of b.guides) expect(x.path.startsWith('/es/')).toBe(lang === 'es');
    }
  });
  it('features six guides per language for the home page', () => {
    expect(featuredGuides('en')).toHaveLength(6);
    expect(featuredGuides('es')).toHaveLength(6);
  });
  it('gives every page a Related list from its own block, without itself', () => {
    for (const lang of ['en', 'es'] as GuideLang[]) {
      for (const b of guideBlocks(lang)) for (const x of b.guides) {
        const rel = relatedGuides(x.path, lang);
        expect(rel.length).toBe(Math.min(4, b.guides.length - 1));
        expect(rel.map((r) => r.path)).not.toContain(x.path);
        for (const r of rel) expect(b.guides.map((s) => s.path)).toContain(r.path);
      }
    }
  });
  it('puts siblings the body already links to last', () => {
    const rel = relatedGuides('/raycast-alternative/', 'en', 'see [x](/ferrocast-vs-raycast/)');
    expect(rel.map((r) => r.path)).not.toContain('/ferrocast-vs-raycast/');
    expect(rel).toHaveLength(4);
  });
});
