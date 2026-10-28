import { describe, expect, it } from 'vitest';
import { es, page } from './alfred-alt';

describe('Alfred alternative copy', () => {
  it('uses its URL and a description of snippet length', () => {
    expect(page.path).toBe('/alfred-alternative/');
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('has a cell for every column and keeps Ferrocast gaps in the table', () => {
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
    const ferrocast = Object.fromEntries(page.limits.rows.map((r) => [r[0], r[2]]));
    expect(ferrocast['Settings sync between Macs']).toMatch(/iCloud Drive/);
    expect(ferrocast['Settings sync between Macs']).not.toMatch(/^No sync/);
    expect(ferrocast['Automation']).toMatch(/no visual workflow editor/);
  });

  it('never says Alfred "does not have" something and gives no Alfred speed or memory numbers', () => {
    const alfred = JSON.stringify(page.limits.rows.map((r) => r[1]));
    expect(alfred).not.toMatch(/does not have|doesn't have|lacks/i);
    expect(alfred).not.toMatch(/\d+\s?(ms|MB)\b/);
  });
});

describe('Alfred alternative copy (Spanish)', () => {
  it('is the sister of the English page with a snippet-length description and the same shape', () => {
    expect(es.path).toBe('/es/alternativa-a-alfred/');
    expect(es.description.length).toBeGreaterThanOrEqual(120);
    expect(es.description.length).toBeLessThanOrEqual(155);
    expect(es.title.length).toBeLessThanOrEqual(70);
    for (const row of es.limits.rows) expect(row).toHaveLength(es.limits.cols.length);
    expect(es.limits.rows).toHaveLength(page.limits.rows.length);
    expect(es.steps.items).toHaveLength(page.steps.items.length);
    expect(es.faq.items).toHaveLength(page.faq.items.length);
    expect(page.more.links).toContainEqual(['Spanish version', '/es/alternativa-a-alfred/']);
    expect(es.more.links).toContainEqual(['English version', '/alfred-alternative/']);
  });

  it('keeps the same Alfred facts as the English page: price, requirements, Powerpack features', () => {
    const alfred = JSON.stringify(es.limits.rows.map((r) => r[1]));
    expect(alfred).toContain('34 £');
    expect(alfred).toContain('macOS 10.14');
    expect(alfred).toMatch(/Powerpack/);
    expect(JSON.stringify(page.limits.rows.map((r) => r[1]))).toContain('£34');
    expect(JSON.stringify(es.lead)).toContain('Alfred 6');
  });

  it('never says Alfred "no tiene" something, gives no Alfred speed or memory numbers and avoids voseo', () => {
    const alfred = JSON.stringify(es.limits.rows.map((r) => r[1]));
    expect(alfred).not.toMatch(/no tiene|carece|no incluye/i);
    expect(alfred).not.toMatch(/\d+\s?(ms|MB)\b/);
    expect(JSON.stringify(es)).not.toMatch(/más rápid|\bRAM\b/i);
    expect(JSON.stringify(es)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
