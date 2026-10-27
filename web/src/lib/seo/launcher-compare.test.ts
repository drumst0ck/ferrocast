import { describe, expect, it } from 'vitest';
import { page } from './launcher-compare';

describe('Raycast vs Alfred vs Spotlight copy', () => {
  it('uses the brief URL, title and H1', () => {
    expect(page.path).toBe('/es/raycast-vs-alfred-vs-spotlight/');
    expect(page.title).toBe('Raycast vs Alfred vs Spotlight en macOS Tahoe: cuál elegir | Ferrocast');
    expect(page.h1).toBe('Raycast, Alfred o Spotlight: qué lanzador elegir en tu Mac');
  });

  it('keeps the meta description within 140-160 characters', () => {
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('has five columns in every table row', () => {
    for (const row of page.table.rows) expect(row).toHaveLength(5);
  });

  it('never says a rival "no tiene" something and gives no rival speed or memory numbers', () => {
    const rivals = JSON.stringify([page.table.rows.map((r) => r.slice(1, 4)), page.apps.items.slice(0, 3), page.faq.items]);
    expect(rivals).not.toMatch(/no tiene/i);
    expect(rivals).not.toMatch(/\b\d+(?:[.,]\d+)?\s?(?:ms|MB de RAM)\b/);
  });

  it('discloses that the author makes Ferrocast', () => {
    expect(page.lead).toMatch(/escribo Ferrocast/);
  });

  it('never uses voseo', () => {
    expect(JSON.stringify(page)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
