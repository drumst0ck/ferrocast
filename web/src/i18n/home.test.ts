import { describe, expect, it } from 'vitest';
import { en } from './home.en';
import { es } from './home.es';

/** Flatten to "path:type" (arrays keep their length) so any missing key, extra key or length drift shows up. */
function shape(v: unknown, path = ''): string[] {
  if (Array.isArray(v)) return [`${path}[len=${v.length}]`, ...v.flatMap((x, i) => shape(x, `${path}[${i}]`))];
  if (v && typeof v === 'object') return Object.entries(v).flatMap(([k, x]) => shape(x, path ? `${path}.${k}` : k));
  return [`${path}:${typeof v}`];
}
const placeholders = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join(',');
function walk(a: unknown, b: unknown, path: string, out: string[]) {
  if (typeof a === 'string' && typeof b === 'string') { if (placeholders(a) !== placeholders(b)) out.push(path); return; }
  if (Array.isArray(a) && Array.isArray(b)) { a.forEach((x, i) => walk(x, b[i], `${path}[${i}]`, out)); return; }
  if (a && b && typeof a === 'object' && typeof b === 'object') for (const k of Object.keys(a)) walk((a as any)[k], (b as any)[k], `${path}.${k}`, out);
}

describe('home copy', () => {
  it('English and Spanish have identical key sets and array lengths', () => {
    expect(shape(es)).toEqual(shape(en));
  });
  it('uses the same {placeholders} in both languages', () => {
    const bad: string[] = [];
    walk(en, es, '', bad);
    expect(bad).toEqual([]);
  });
  it('has meta descriptions that fit 140-155 characters once filled', () => {
    for (const t of [en, es]) {
      const d = t.meta.description.replace('{pairs}', '275').replace('{tools}', '26');
      expect(d.length).toBeGreaterThanOrEqual(140);
      expect(d.length).toBeLessThanOrEqual(155);
    }
  });
  it('never uses voseo in the Spanish copy', () => {
    const text = JSON.stringify(es);
    expect(text).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
