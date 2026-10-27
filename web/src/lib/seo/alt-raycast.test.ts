import { describe, expect, it } from 'vitest';
import { en, es, inline, plain } from './alt-raycast';

describe('Raycast alternative copy', () => {
  it('uses the brief title, meta description, H1 and URL for the Spanish page', () => {
    expect(es.path).toBe('/es/alternativa-a-raycast/');
    expect(es.title).toBe('Alternativa gratis a Raycast para Mac, de código abierto | Ferrocast');
    expect(es.description).toBe(
      'Ferrocast es un lanzador gratis y MIT para Mac: Script Commands de Raycast, IA local con Ollama y 275 conversiones sin subir archivos. Lo que falta, también.',
    );
    expect(es.h1).toBe('Una alternativa a Raycast gratis y de código abierto para Mac');
    expect(en.path).toBe('/raycast-alternative/');
  });

  it('keeps meta descriptions within 140-160 characters', () => {
    for (const t of [en, es]) {
      expect(t.description.length).toBeGreaterThanOrEqual(140);
      expect(t.description.length).toBeLessThanOrEqual(160);
    }
  });

  it('has the ten brief FAQ entries and matching structure in both languages', () => {
    expect(es.faq.items).toHaveLength(10);
    expect(en.faq.items).toHaveLength(10);
    expect(en.table.rows).toHaveLength(es.table.rows.length);
    expect(en.install.steps).toHaveLength(es.install.steps.length);
    for (const row of [...en.table.rows, ...es.table.rows]) expect(row).toHaveLength(4);
  });

  it('never claims rival speed or memory numbers and never says a rival "does not have" something', () => {
    const text = JSON.stringify([en.table, es.table, en.price, es.price]);
    expect(text).not.toMatch(/\b\d+(?:[.,]\d+)?\s?(?:ms|MB)\b.*(?:Raycast|Alfred)/);
    expect(text).not.toMatch(/no tiene|does not have|doesn't have/i);
  });

  it('never uses voseo in the Spanish copy', () => {
    expect(JSON.stringify(es)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });

  it('renders inline links, bold and code, and escapes everything else', () => {
    expect(inline('a <b> [x](/y/) **z** `c`')).toBe('a &lt;b&gt; <a class="tl" href="/y/">x</a> <strong>z</strong> <code>c</code>');
    expect(inline('[g](https://github.com/)')).toContain('rel="noopener"');
    expect(plain('see [x](/y/) **z** `c`')).toBe('see x z c');
  });
});
