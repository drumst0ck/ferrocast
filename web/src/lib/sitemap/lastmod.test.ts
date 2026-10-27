import { describe, expect, it } from 'vitest';
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
});
