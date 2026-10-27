import { describe, expect, it } from 'vitest';
import { en, es } from './compress-video';

const all = (t: typeof en) => JSON.stringify(t);

describe('compress video to size copy', () => {
  it('uses the brief URLs, titles and H1s', () => {
    expect(en.path).toBe('/compress-video-to-size/');
    expect(es.path).toBe('/es/comprimir-video-a-un-tamano/');
    expect(en.title).toBe('Compress a video to a target size on Mac (Discord, email) | Ferrocast');
    expect(es.title).toBe('Comprimir un vídeo a un tamaño concreto en Mac, sin subirlo | Ferrocast');
    expect(en.h1).toBe('Compress a video to a target size on your Mac');
    expect(es.h1).toBe('Comprimir un vídeo a un tamaño concreto en tu Mac');
  });

  it('keeps meta descriptions within 140-160 characters', () => {
    for (const t of [en, es]) {
      expect(t.description.length).toBeGreaterThanOrEqual(140);
      expect(t.description.length).toBeLessThanOrEqual(160);
    }
  });

  it('has the same structure in both languages', () => {
    expect(es.steps.items).toHaveLength(en.steps.items.length);
    expect(es.faq.items).toHaveLength(en.faq.items.length);
    expect(es.limits.rows).toHaveLength(en.limits.rows.length);
    expect(es.sections.map((s) => s.list?.length ?? 0)).toEqual(en.sections.map((s) => s.list?.length ?? 0));
  });

  it('never promises an exact size or no quality loss', () => {
    // The FAQ may ask the question ("Will it hit the exact size?") to answer it honestly; claims may not.
    const claims = (t: typeof en) => JSON.stringify([t.title, t.description, t.h1, t.lead, t.steps, t.sections, t.faq.items.map(([, a]) => a)]);
    expect(claims(en)).not.toMatch(/exact size|exactly the size|without (losing|quality loss)|lossless/i);
    expect(claims(es)).not.toMatch(/tamaño exacto|exactamente del tamaño|sin perder calidad/i);
  });

  it('states the current Discord limit (20 MiB) and suggests a target under it', () => {
    for (const t of [en, es]) {
      expect(t.limits.rows[0][1]).toContain('20 MiB');
      expect(Number(t.limits.rows[0][2].replace(/\D/g, '')) * 1024).toBeLessThan(20 * 1024 * 1024);
      expect(Number(t.limits.rows[1][2].replace(/\D/g, '')) * 1024).toBeLessThan(25 * 1000 * 1000);
    }
  });

  it('never uses voseo in the Spanish copy', () => {
    expect(all(es)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí|escribí)\b/i);
  });
});
