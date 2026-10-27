import { describe, expect, it } from 'vitest';
import { allPairs } from './convert';
import { page } from './file-converter';

describe('free file converter copy', () => {
  it('has a title and description of snippet length', () => {
    expect(page.title.length).toBeLessThanOrEqual(65);
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('links only to conversion pages that exist', () => {
    const slugs = new Set(allPairs().map((p) => p.slug));
    const links = JSON.stringify(page).match(/\/convert\/[a-z0-9-]+\//g) ?? [];
    expect(links.length).toBeGreaterThan(0);
    for (const l of links) expect(slugs.has(l.slice('/convert/'.length, -1)), l).toBe(true);
  });

  it('has a cell for every column and gives no speed or quality comparison', () => {
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
    expect(JSON.stringify(page)).not.toMatch(/faster|better quality|higher quality/i);
  });
});
