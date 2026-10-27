import { describe, expect, it } from 'vitest';
import { en, es } from './screen-recorder';

describe('screen recorder pages', () => {
  it('have descriptions of snippet length and the same structure in both languages', () => {
    for (const t of [en, es]) {
      expect(t.description.length).toBeGreaterThanOrEqual(140);
      expect(t.description.length).toBeLessThanOrEqual(160);
    }
    expect(es.limits.rows).toHaveLength(en.limits.rows.length);
    expect(es.steps.items).toHaveLength(en.steps.items.length);
    expect(es.faq.items).toHaveLength(en.faq.items.length);
  });

  it('quote the auto zoom timings and GIF limits from the docs', () => {
    expect(JSON.stringify(en)).toContain('0.4 s before');
    expect(JSON.stringify(es)).toContain('0,4 s antes');
    expect(JSON.stringify(en)).toContain('about 20 seconds at 960 px');
  });

  it('never compare quality or speed with Screen Studio', () => {
    for (const t of [en, es]) expect(JSON.stringify(t)).not.toMatch(/better quality|faster than|más rápid|mejor calidad/i);
  });

  it('never use voseo', () => {
    expect(JSON.stringify(es)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
