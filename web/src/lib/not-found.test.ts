import { describe, expect, it } from 'vitest';
import { notFoundCopy, notFoundLang } from './not-found';

describe('404 language', () => {
  it('uses Spanish under /es/ only', () => {
    expect(notFoundLang('/es/nada/')).toBe('es');
    expect(notFoundLang('/es')).toBe('es');
    expect(notFoundLang('/espresso/')).toBe('en');
    expect(notFoundLang('/docs/x/')).toBe('en');
  });
  it('has a Spanish copy for every key', () => {
    for (const k of Object.keys(notFoundCopy.en) as (keyof typeof notFoundCopy.en)[]) expect(notFoundCopy.es[k]).not.toBe(notFoundCopy.en[k]);
  });
});
