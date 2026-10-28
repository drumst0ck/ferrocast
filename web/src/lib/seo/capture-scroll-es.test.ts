import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { page } from './capture-scroll-es';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('scrolling capture page (ES)', () => {
  it('has a description of snippet length and a cell for every column', () => {
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
    expect(page.title.length).toBeLessThanOrEqual(65);
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
  });

  it('quotes the scrolling capture facts from the docs and the app', () => {
    const doc = read('web/src/content/docs/docs/launcher/screen-capture.md');
    for (const s of ['after 60 screens', '`Scrolling capture stopped at 60 screens`', 'Copied N characters', 'No text found', 'Capture Scrolling also needs Accessibility', '**macOS 14 or later.**']) {
      expect(doc, s).toContain(s);
    }
    expect(read('crates/app/src/capture_ctl.rs')).toContain('post_scroll');
  });

  it('only claims vertical scrolling, because the helper posts one vertical delta', () => {
    const src = read('crates/macos/src/scroll.rs');
    expect(src).toContain('pub fn post_scroll(x: f64, y: f64, dy: i32)');
    expect(src).toContain('A negative `dy` moves the');
    expect(JSON.stringify(page)).not.toMatch(/funciona en horizontal\.|scroll horizontal (sí|funciona)/i);
    expect(JSON.stringify(page)).toContain('no afirmo que funcione en horizontal');
  });

  it('does not say macOS cannot do it, only what Apple’s guide lists', () => {
    const s = JSON.stringify(page);
    expect(s).not.toMatch(/macOS no (puede|permite|lo hace)|no tiene captura/i);
    expect(s).toContain('no menciona la captura con desplazamiento');
  });

  it('never uses voseo or compares with other apps on speed or quality', () => {
    const s = JSON.stringify(page);
    expect(s).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
    expect(s).not.toMatch(/mejor que|más rápid|\bRAM\b/);
  });

  it('links only to pages that exist', () => {
    const paths = ['/es/grabar-pantalla-mac-zoom-automatico', '/es/difuminar-video-mac', '/es/alternativa-a-raycast', '/cleanshot-alternative'];
    for (const [, href] of page.more.links) {
      const p = href.replace(/\/$/, '');
      if (href.startsWith('/es/docs/') || href === '/es/facts/') continue;
      expect(paths, href).toContain(p);
      const f = p.startsWith('/es/') ? `web/src/pages${p}.astro` : `web/src/pages${p}.astro`;
      expect(() => read(f), f).not.toThrow();
    }
  });
});
