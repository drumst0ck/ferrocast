import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { COMMANDS, page as en } from './windows';
import { page } from './windows-es';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('window management page (ES)', () => {
  it('has a description of snippet length and a cell for every column', () => {
    expect(page.description.length).toBeGreaterThanOrEqual(120);
    expect(page.description.length).toBeLessThanOrEqual(160);
    expect(page.title.length).toBeLessThanOrEqual(70);
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
  });

  it('lists every command windows.rs defines, once', () => {
    const src = read('crates/macos/src/windows.rs');
    for (const c of COMMANDS) expect(src, c).toContain(`"${c}"`);
    const list = page.sections[0].paras?.[0] ?? '';
    for (const c of COMMANDS) expect(list, c).toContain(c);
    expect(COMMANDS).toHaveLength(40);
  });

  it('mirrors the English FAQ and steps', () => {
    expect(page.faq.items.length).toBeGreaterThanOrEqual(en.faq.items.length);
    expect(page.steps.items).toHaveLength(en.steps.items.length);
  });

  it('states automatic tiling as off by default and in 0.4.0, as the docs and changelog do', () => {
    expect(read('web/src/content/docs/docs/launcher/tiling.md')).toContain('**off by default**');
    expect(read('CHANGELOG.md')).toMatch(/## \[0\.4\.0\][\s\S]*\*\*Automatic window tiling\*\*/);
  });

  it('never says a rival lacks something without a source, and never uses voseo', () => {
    const s = JSON.stringify(page);
    expect(s).not.toMatch(/no tiene|no incluye|faltan?\b|más rápid|\bRAM\b/i);
    expect(s).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
