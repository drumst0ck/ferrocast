import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { page } from './cleanshot-alt';
import { allTools } from './tools';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('CleanShot alternative page', () => {
  it('has a description of snippet length and a cell for every column', () => {
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
    expect(page.title.length).toBeLessThanOrEqual(65);
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
  });

  it('quotes Ferrocast capture facts from the docs', () => {
    const doc = read('web/src/content/docs/docs/launcher/screen-capture.md');
    for (const s of ['**macOS 14 or later.**', 'after 60 screens', 'from 25 % to 400 %', 'Copied N characters', 'starts 0.4 s before', '**Annotate…**', '`~/Pictures/Ferrocast Captures`']) {
      expect(doc, s).toContain(s);
    }
    expect(JSON.stringify(page)).toContain('0.4 s before');
  });

  it('claims vertical scrolling only, because the helper sends vertical events', () => {
    const src = read('crates/macos/src/scroll.rs');
    expect(src).toContain('pub fn post_scroll(x: f64, y: f64, dy: i32)');
    expect(src).not.toMatch(/\bdx\b/);
    expect(JSON.stringify(page)).not.toMatch(/horizontal(ly)? scroll(ing)? (capture )?(is )?(supported|works)/i);
  });

  it('dates the CleanShot prices and never says it replaces CleanShot or compares speed', () => {
    const s = JSON.stringify(page);
    expect(s).toContain('28 October 2026');
    expect(s).toContain('$35');
    expect(s).toContain('$19 a year');
    expect(s).not.toMatch(/replaces? CleanShot|everything CleanShot|faster than|better quality|\bRAM\b/i);
    expect(s).toContain('What CleanShot X does that Ferrocast doesn’t');
  });

  it('links only to pages that exist', () => {
    for (const [, href] of page.more.links) {
      const p = href.replace(/\/$/, '');
      if (href.startsWith('/docs/')) continue;
      if (href.startsWith('/tools/')) { expect(allTools().map((t) => t.slug), href).toContain(p.split('/').pop()); continue; }
      const files = [`web/src/pages${p}.astro`, `web/src/pages${p}/index.astro`, `web/src/pages${p}/[slug].astro`];
      expect(files.some((f) => { try { read(f); return true; } catch { return false; } }), href).toBe(true);
    }
  });
});
