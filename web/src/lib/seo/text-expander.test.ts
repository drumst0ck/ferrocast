import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { en, es } from './text-expander';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('text expander pages', () => {
  it('have descriptions of snippet length and the same structure in both languages', () => {
    for (const t of [en, es]) {
      expect(t.description.length).toBeGreaterThanOrEqual(140);
      expect(t.description.length).toBeLessThanOrEqual(160);
      expect(t.title.length).toBeLessThanOrEqual(70);
      for (const row of t.limits.rows) expect(row).toHaveLength(t.limits.cols.length);
    }
    expect(es.limits.rows).toHaveLength(en.limits.rows.length);
    expect(es.steps.items).toHaveLength(en.steps.items.length);
    expect(es.sections).toHaveLength(en.sections.length);
    expect(es.faq.items).toHaveLength(en.faq.items.length);
  });

  it('only documents placeholders and formats the engine implements', () => {
    const src = read('crates/core/src/placeholders.rs');
    for (const k of ['"clipboard"', '"snippet"', '"cursor"', '"date"', '"time"', '"day"']) expect(src).toContain(k);
    // the examples used in the copy are asserted by the engine's own tests
    expect(src).toContain('{date format=\\"yyyy-MM-dd\\" offset=\\"+3M -5d\\"}');
    expect(src).toContain('{clipboard offset=1}');
    expect(src).toContain('console.log({cursor})');
    expect(src).toContain('{snippet name=\\"sig\\"}');
  });

  it('describes the keyword prefix rule and the Raycast import as the code does', () => {
    expect(read('crates/app/src/draw.rs')).toMatch(/s\.keyword\.starts_with\(&kw\) \|\| kw\.starts_with\(&s\.keyword\)/);
    expect(read('crates/app/src/items.rs')).toContain('"Import Snippets"');
    const imp = read('crates/core/src/import.rs');
    expect(imp).toContain('Raycast');
    expect(imp).toContain('skipping exact duplicates');
  });

  it('does not claim fill-in forms, TextExpander import or sync as unreleased', () => {
    for (const t of [en, es]) {
      const s = JSON.stringify(t);
      expect(s).not.toMatch(/coming in the next version|llega en la próxima versión/i);
      expect(s).not.toMatch(/(can|puedes) import(ar)? (your |tus )?(snippets )?(from|desde) TextExpander/i);
    }
    expect(JSON.stringify(en)).toContain('It has no fill-in forms');
    expect(JSON.stringify(es)).toContain('No tiene formularios de relleno');
    expect(read('CHANGELOG.md')).toMatch(/## \[0\.4\.0\][\s\S]*\*\*Sync with iCloud\*\*/);
  });

  it('never compare speed or memory and never use voseo', () => {
    for (const t of [en, es]) expect(JSON.stringify(t)).not.toMatch(/faster than|más rápid|\bRAM\b|memory usage/);
    expect(JSON.stringify(es)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
