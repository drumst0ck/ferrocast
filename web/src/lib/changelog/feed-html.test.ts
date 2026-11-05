import { describe, expect, it } from 'vitest';
import { jsonFeedHtml, rssFeedHtml } from './feed-html';
import type { ReleaseView } from './query';

const entry = (title: string | null, category: 'added' | 'fixed' = 'added') => ({
  id: 1, category, slug: 's', title, bodyMd: 'x', bodyHtml: '<em>ok</em>', docsUrl: null, position: 0, updatedAt: new Date(0), tags: [],
});
const release = (titles: (string | null)[]): ReleaseView => ({ id: 1, version: '1.0.0', releasedOn: '2026-01-01', summary: null, entries: titles.map((t) => entry(t)) });

describe('feed HTML escapes entry titles', () => {
  const evil = 'A <img src=x onerror=alert(1)> & "B"';
  it('RSS content', () => {
    const html = rssFeedHtml(release([evil]));
    expect(html).not.toContain('<img');
    expect(html).toContain('<strong>A &lt;img src=x onerror=alert(1)&gt; &amp; &quot;B&quot;</strong>');
    expect(html).toContain('<em>ok</em>');
  });
  it('JSON feed content', () => {
    const html = jsonFeedHtml(release([evil]));
    expect(html).not.toContain('<img');
    expect(html).toContain('<strong>A &lt;img src=x onerror=alert(1)&gt; &amp; &quot;B&quot;</strong>');
  });
  it('untitled entries and plain titles still render', () => {
    expect(rssFeedHtml(release(['Plain', null]))).toContain('<li><strong>Plain</strong>: <em>ok</em></li><li><em>ok</em></li>');
    expect(jsonFeedHtml(release(['Plain']))).toBe('<p><strong>Plain</strong>: <em>ok</em></p>');
  });
});
