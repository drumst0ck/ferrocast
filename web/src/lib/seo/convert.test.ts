import { describe, expect, it } from 'vitest';
import { allPairs, familyHubs, installCommand, pairFacts, pairMeta, relatedPairs } from './convert';

describe('conversion pages data', () => {
  const pairs = allPairs();
  it('covers all 275 pairs with unique slugs', () => {
    expect(pairs).toHaveLength(275);
    expect(new Set(pairs.map((p) => p.slug)).size).toBe(275);
    expect(pairs.find((p) => p.slug === 'heic-to-jpg')).toBeTruthy();
  });
  it('has 9 family hubs whose slugs never collide with pair slugs', () => {
    const hubs = familyHubs();
    expect(hubs).toHaveLength(9);
    const slugs = new Set(pairs.map((p) => p.slug));
    for (const h of hubs) expect(slugs.has(h.id)).toBe(false);
  });
  it('gives every page a unique title (<=60) and a unique description (120-155)', () => {
    const titles = new Set<string>();
    const descs = new Set<string>();
    for (const p of pairs) {
      const m = pairMeta(p);
      expect(m.title.length, m.title).toBeLessThanOrEqual(60);
      expect(m.description.length, m.description).toBeGreaterThanOrEqual(120);
      expect(m.description.length, m.description).toBeLessThanOrEqual(155);
      titles.add(m.title);
      descs.add(m.description);
    }
    expect(titles.size).toBe(275);
    expect(descs.size).toBe(275);
  });
  it('only claims an optional tool when the data says the pair needs one', () => {
    const none = pairs.find((p) => p.needs === null)!;
    const needy = pairs.find((p) => p.needs !== null)!;
    expect(pairMeta(none).description).not.toMatch(/needs/i);
    expect(pairMeta(needy).description).toContain(needy.needs!);
    expect(pairFacts(none).requirement).toMatch(/no extra tool/i);
    expect(pairFacts(needy).requirement).toContain(installCommand(needy.needs!));
  });
  it('maps each optional tool to the install command the app shows', () => {
    expect(installCommand('ffmpeg')).toBe('brew install ffmpeg');
    expect(installCommand('LibreOffice')).toBe('brew install --cask libreoffice');
    expect(installCommand('cwebp')).toBe('brew install webp');
  });
  it('keeps the documented limits on the pairs they apply to', () => {
    const limits = (slug: string) => pairFacts(pairs.find((p) => p.slug === slug)!).limits.join(' ');
    expect(limits('docx-to-pdf')).toMatch(/layout/i);
    expect(limits('docx-to-pages')).toMatch(/iWork '09/);
    expect(limits('pdf-to-png')).toMatch(/300 DPI/);
    expect(limits('heic-to-jpg')).not.toMatch(/layout|iWork|300 DPI/);
  });
  it('links related pairs that exist', () => {
    const slugs = new Set(pairs.map((p) => p.slug));
    const r = relatedPairs(pairs.find((p) => p.slug === 'jpg-to-png')!);
    expect(r.reverse?.slug).toBe('png-to-jpg');
    for (const p of [...r.sameInput, ...r.sameOutput]) expect(slugs.has(p.slug)).toBe(true);
    expect(r.sameInput.length).toBeGreaterThan(0);
  });
});
