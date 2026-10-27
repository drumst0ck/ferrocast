import { describe, expect, it } from 'vitest';
import { esFor, esPairs, hub, pairOf } from './convert-es';

describe('Spanish conversion pages', () => {
  it('covers the five pairs of the brief, each backed by a real English pair with the same requirement', () => {
    expect(esPairs.map((e) => e.slug)).toEqual(['heic-a-jpg', 'webp-a-jpg', 'mov-a-mp4', 'video-a-gif', 'pdf-a-word']);
    for (const e of esPairs) expect(pairOf(e).slug).toBe(e.en);
    expect(esFor('heic-to-jpg')?.slug).toBe('heic-a-jpg');
    expect(esFor('jpg-to-png')).toBeUndefined();
  });

  it('keeps titles and meta descriptions to search snippet length', () => {
    for (const t of [hub, ...esPairs]) {
      expect(t.title.length).toBeLessThanOrEqual(75);
      expect(t.description.length).toBeGreaterThanOrEqual(130);
      expect(t.description.length).toBeLessThanOrEqual(160);
    }
  });

  it('never promises kept metadata or no quality loss', () => {
    expect(JSON.stringify(esPairs)).not.toMatch(/sin perder calidad|conserva (los )?metadatos|calidad original/i);
  });

  it('never uses voseo', () => {
    expect(JSON.stringify([hub, esPairs])).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí|soltá)\b/i);
  });
});
