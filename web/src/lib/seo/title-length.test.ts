import { describe, expect, it } from 'vitest';
import { longTitles, MAX_TITLE } from './title-length';
import { page as alfred } from './alfred-alt';
import { es as altRaycastEs } from './alt-raycast';
import { page as vs } from './vs-raycast';
import { page as clipboardEs } from './clipboard-es';
import { pairGuides } from './pair-guides';
import { esPairs } from './convert-es';

const html = (t: string) => `<html><head><title>${t}</title></head><body></body></html>`;

describe('longTitles', () => {
  const long = 'x'.repeat(MAX_TITLE + 1);
  it('flags non-docs pages with a title over 60 characters', () => {
    const r = longTitles([
      { path: '/a/index.html', html: html(long) },
      { path: '/b/index.html', html: html('short') },
    ]);
    expect(r).toEqual([{ path: '/a/index.html', title: long, length: 61 }]);
  });
  it('ignores docs pages in both languages', () => {
    expect(longTitles([
      { path: '/docs/x/index.html', html: html(long) },
      { path: '/es/docs/x/index.html', html: html(long) },
    ])).toEqual([]);
  });
  it('decodes HTML entities before counting', () => {
    const t = 'a&amp;b'.padEnd(0) + 'y'.repeat(57); // decoded: 'a&b' + 57 = 60
    expect(longTitles([{ path: '/c/index.html', html: html(t) }])).toEqual([]);
  });
  it('skips pages without a title', () => {
    expect(longTitles([{ path: '/d/index.html', html: '<html></html>' }])).toEqual([]);
  });
});

describe('the six shortened titles', () => {
  const titles = [
    alfred.title,
    altRaycastEs.title,
    vs.title,
    clipboardEs.title,
    pairGuides['md-to-pages'].title,
    esPairs.find((p) => p.slug === 'heic-a-jpg')!.title,
  ];
  it('are 60 characters or fewer', () => {
    for (const t of titles) expect(t.length, t).toBeLessThanOrEqual(60);
  });
});
