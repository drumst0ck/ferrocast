import { describe, expect, it } from 'vitest';
import manifest from '../../data/lastmod.json';
import { lastmodForPageUrl } from './lastmod';

describe('lastmodForPageUrl', () => {
  it('returns a Date for home, docs, and convert URLs without throwing', () => {
    for (const url of [
      'https://ferrocast.app/',
      'https://ferrocast.app/docs/files/conversions/',
      'https://ferrocast.app/convert/heic-to-jpg/',
    ]) {
      const d = lastmodForPageUrl(url);
      expect(d).toBeInstanceOf(Date);
      expect(Number.isNaN(d.getTime())).toBe(false);
    }
  });

  it('manifest has many distinct git dates for Docker builds without .git', () => {
    expect(new Set(Object.values(manifest)).size).toBeGreaterThan(5);
  });

  it('doc URL lastmod follows manifest content file date', () => {
    const rel = 'src/content/docs/docs/files/conversions.md';
    const expected = manifest[rel as keyof typeof manifest];
    expect(expected).toBeTruthy();
    const d = lastmodForPageUrl('https://ferrocast.app/docs/files/conversions/');
    expect(d.toISOString()).toBe(new Date(expected!).toISOString());
  });
});
