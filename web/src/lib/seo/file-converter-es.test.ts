import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { counts } from '../../data/status';
import { plain } from './markup';
import { H3_COUNT, OPTIONAL_H, optionalTools, page } from './file-converter-es';

const words = (s: string) => plain(s).split(/\s+/).filter(Boolean).length;

describe('Spanish free file converter page', () => {
  it('uses the brief URL, title, meta and H1', () => {
    expect(page.path).toBe('/es/convertidor-de-archivos-gratis-mac/');
    expect(page.title).toBe('Convertidor de archivos gratis para Mac, sin subir nada | Ferrocast');
    expect(page.h1).toBe('Un convertidor de archivos gratis para Mac que no sube tus archivos');
    expect(page.description).toBe(
      'Convierte imágenes, vídeo, audio, PDF y documentos en tu Mac sin subirlos: 275 conversiones y 26 herramientas, gratis y MIT. Comparado con Permute y CloudConvert.',
    );
    expect(page.h1).not.toBe('Convertir archivos en tu Mac, sin subirlos');
    const c = counts();
    expect(c).toMatchObject({ pairs: 275, inputs: 46, tools: 26 });
  });

  it('stays inside the brief length and does not use banned claims', () => {
    const n = words(
      [
        ...page.lead,
        ...page.limits.rows.flat(),
        page.limits.note,
        ...page.steps.items.flat(),
        ...page.sections.flatMap((s) => [s.h, ...(s.paras ?? []), ...(s.list ?? [])]),
        ...optionalTools.rows.flat(),
        ...page.faq.items.flat(),
      ].join(' '),
    );
    expect(n).toBeGreaterThanOrEqual(1100);
    expect(n).toBeLessThanOrEqual(1600);
    const text = JSON.stringify(page);
    expect(text).not.toMatch(/el único|el mejor|sin límites/i);
    expect(text).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí|soltá)\b/i);
    expect(text).not.toMatch(/iCloud|Clear History|Camomilla|AltFormr|nxtconvert|difuminar-video/i);
    expect(text).not.toMatch(/\/es\/files\/|\/es\/tools\//);
  });

  it('keeps the citable answers and the checked prices', () => {
    expect(page.faq.items[0][1]).toMatch(/no pide cuenta/);
    expect(page.faq.items[1][1]).toMatch(/sips o avconvert/);
    expect(page.faq.items[2][1]).toMatch(/Permute 3 funciona en Mac con Intel/);
    expect(page.faq.items[5][1]).toMatch(/17 \$/);
    expect(page.faq.items[5][1]).toMatch(/9 \$ al mes/);
    expect(page.limits.rows[1][2]).toMatch(/14,99 \$/);
    expect(page.sections[H3_COUNT].h).not.toBe(OPTIONAL_H);
    expect(page.sections.some((s) => s.h === OPTIONAL_H)).toBe(true);
  });

  it('marks the app as a free macOS utility in the page', () => {
    const src = readFileSync('src/components/seo/FileConverterEsPage.astro', 'utf8');
    expect(src).toContain("applicationCategory: 'UtilitiesApplication'");
    expect(src).toContain("operatingSystem: 'macOS 13+'");
    expect(src).toContain("price: '0'");
    expect(src).not.toContain('AggregateRating');
  });
});
