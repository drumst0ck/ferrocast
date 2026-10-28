import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { plain } from './markup';
import { page } from './organize-pdf';

const words = (s: string) => plain(s).split(/\s+/).filter(Boolean).length;

describe('organize PDF page', () => {
  it('uses the brief URL, title, meta and H1', () => {
    expect(page.path).toBe('/tools/organize-pdf/');
    expect(page.title).toBe('PDF Organizer for Mac: Reorder, Rotate, Delete Pages Offline');
    expect(page.description).toBe(
      'Reorder, rotate, duplicate and remove PDF pages on your Mac with thumbnails or one typed command. Free, open source, nothing uploaded, saves a copy.',
    );
    expect(page.h1).toBe('Organize PDF pages on your Mac, offline');
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('says the mouse buttons shipped with the organizer', () => {
    expect(page.can.buttons).toMatch(/◀ Move/);
    expect(page.can.buttons).not.toMatch(/coming in the next version/i);
    const docs = readFileSync('src/content/docs/docs/files/tools/organize-pdf.mdx', 'utf8');
    expect(docs).toMatch(/◀ Move/);
    expect(docs).not.toMatch(/coming in the next version/i);
    expect(docs).toMatch(/\[PDF organizer for Mac\]\(\/tools\/organize-pdf\/\)/);
    const description = docs.match(/description: "([^"]+)"/)?.[1] ?? '';
    expect(description.length).toBeGreaterThanOrEqual(120);
    expect(description.length).toBeLessThanOrEqual(155);
    expect(description.toLowerCase()).not.toContain('organizer');
  });

  it('does not claim the organizer merges PDFs, and links merge elsewhere', () => {
    const body = JSON.stringify([page.lead, page.can, page.steps, page.command, page.limits, page.faq]);
    expect(body).not.toMatch(/organizer (also )?merges|merge PDFs in the organizer/i);
    expect(page.limits.items.join(' ')).toMatch(/\/tools\/merge-pdf\//);
  });

  it('keeps the page in the brief length', () => {
    const n = words(
      [
        page.lead,
        page.can.intro,
        page.can.buttons,
        ...page.can.rows.flat(),
        ...page.steps.items.flat(),
        ...page.command.paras,
        ...page.limits.items,
        ...page.compare.rows.flat(),
        page.compare.note,
        ...page.faq.items.flat(),
      ].join(' '),
    );
    expect(n).toBeGreaterThanOrEqual(700);
    expect(n).toBeLessThanOrEqual(1100);
  });
});
