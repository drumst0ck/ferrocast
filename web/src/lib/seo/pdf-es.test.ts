import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { page } from './pdf-es';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('PDF page (ES)', () => {
  it('has a snippet-length title and description and a cell for every column', () => {
    expect(page.path).toBe('/es/unir-pdf-mac/');
    expect(page.description.length).toBeGreaterThanOrEqual(120);
    expect(page.description.length).toBeLessThanOrEqual(155);
    expect(page.title.length).toBeLessThanOrEqual(70);
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
  });

  it('describes merge, split and compress as the code does', () => {
    const services = read('crates/app/src/services.rs');
    expect(services).toContain('/System/Library/Automator/Combine PDF Pages.action/Contents/MacOS/join');
    expect(services).toContain('&format!("page {i}")');
    const convert = read('crates/core/src/convert.rs');
    expect(convert).toContain('FileTool::Merge => "merged".into()');
    expect(convert).toContain('FileTool::SplitPages => "Split into one PDF per page"');
    const compress = read('crates/core/src/compress.rs');
    for (const s of ['/printer', '/ebook', '/screen']) expect(compress).toContain(`"${s}"`);
    expect(read('crates/app/src/route.rs')).toContain('name compressed.pdf');
    expect(read('crates/app/src/app.rs')).toContain('"Choose order…"');
    expect(JSON.stringify(page)).toContain('Ghostscript');
  });

  it('only offers the typed pages command without ranges, and says the split is one file per page', () => {
    const s = JSON.stringify(page);
    expect(s).toContain('no intervalos');
    expect(s).toContain('`pages 3,4,5`');
    expect(s).not.toMatch(/pages \d+-\d+/);
  });

  it('keeps Vista Previa honest: sourced facts and no "no puede"', () => {
    const preview = JSON.stringify(page.limits.rows.map((r) => r[2]));
    expect(preview).not.toMatch(/no puede|no permite|no tiene|carece/i);
    expect(page.limits.note).toContain('support.apple.com/es-es/guide/preview/');
    expect(page.lead.join(' ')).toMatch(/Vista Previa/);
  });

  it('gives no speed or memory claims and avoids voseo', () => {
    const s = JSON.stringify(page);
    expect(s).not.toMatch(/más rápid|\bRAM\b|segundos|\d+\s?ms\b/i);
    expect(s).not.toMatch(/(?<![\p{L}])(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)(?![\p{L}])/iu);
  });
});
