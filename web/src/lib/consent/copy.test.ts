import { describe, expect, it } from 'vitest';
import { consentCopy } from './copy';
import { policyCopy } from './policy';

describe('consent texts', () => {
  it('have every string in both languages and Spanish differs from English', () => {
    expect(Object.keys(consentCopy.es).sort()).toEqual(Object.keys(consentCopy.en).sort());
    for (const k of ['region', 'title', 'text', 'more', 'accept', 'reject', 'settings'] as const) {
      expect(consentCopy.es[k]).not.toBe(consentCopy.en[k]);
      expect(consentCopy.es[k].length).toBeGreaterThan(0);
    }
    expect(consentCopy.es.settings).toBe('Configuración de cookies');
    expect(consentCopy.en.settings).toBe('Cookie settings');
  });
  it('cookie policy lists the consent key and the GA cookies in both languages', () => {
    for (const lang of ['en', 'es'] as const) {
      const names = policyCopy[lang].rows.map((r) => r[0]).join(' ');
      expect(names).toContain('ferrocast-consent');
      expect(names).toContain('_ga');
      expect(names).toContain('_ga_');
      expect(policyCopy[lang].rows.every((r) => r.length === policyCopy[lang].cols.length)).toBe(true);
    }
    expect(policyCopy.es.path).toBe('/es/cookies/');
  });
  it('lists the docs sidebar state and does not overstate the GA cookie duration', () => {
    for (const lang of ['en', 'es'] as const) {
      const rows = policyCopy[lang].rows;
      const sidebar = rows.find((r) => r[0].includes('sl-sidebar-state'));
      expect(sidebar, lang).toBeDefined();
      expect(sidebar![0]).toMatch(/session storage|almacenamiento de sesión/);
      for (const r of rows.filter((x) => x[0].startsWith('_ga'))) expect(r[2]).toMatch(/13/);
    }
  });
});
