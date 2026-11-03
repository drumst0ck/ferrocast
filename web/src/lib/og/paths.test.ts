import { describe, expect, it } from 'vitest';
import { hasOwnOgImage, ogAlt, ogImagePath, ogText } from './paths';

describe('per-page og:image paths', () => {
  it('maps a canonical URL to a static /og/*.png path', () => {
    expect(ogImagePath('https://ferrocast.app/clipboard-manager-mac/')).toBe('/og/clipboard-manager-mac.png');
    expect(ogImagePath('https://ferrocast.app/es/historial-portapapeles-mac/')).toBe('/og/es--historial-portapapeles-mac.png');
    expect(ogImagePath('https://ferrocast.app/tools/split-pdf/')).toBe('/og/tools--split-pdf.png');
  });
  it('is unique per page', () => {
    const urls = ['/a/', '/es/a/', '/tools/a/', '/convert/a/', '/es/convertir/a/'].map((p) => ogImagePath(`https://ferrocast.app${p}`));
    expect(new Set(urls).size).toBe(urls.length);
  });
  it('keeps the generic image for the homes and for pages without canonical', () => {
    expect(hasOwnOgImage('https://ferrocast.app/')).toBe(false);
    expect(hasOwnOgImage('https://ferrocast.app/es/')).toBe(false);
    expect(hasOwnOgImage(undefined)).toBe(false);
    expect(hasOwnOgImage('https://ferrocast.app/facts/')).toBe(true);
  });
  it('keeps the generic image for server-rendered pages, whose PNG the build never writes', () => {
    expect(hasOwnOgImage('https://ferrocast.app/changelog/', false)).toBe(false);
    expect(hasOwnOgImage('https://ferrocast.app/changelog/0.5.0/', false)).toBe(false);
    expect(hasOwnOgImage('https://ferrocast.app/changelog/', true)).toBe(true);
  });
  it('uses the title without the brand suffix as image text and alt', () => {
    expect(ogText('Free Clipboard Manager for Mac | Ferrocast')).toBe('Free Clipboard Manager for Mac');
    expect(ogText('Ferrocast')).toBe('Ferrocast');
    expect(ogAlt('Free Clipboard Manager for Mac | Ferrocast')).toContain('Free Clipboard Manager for Mac');
  });
});
