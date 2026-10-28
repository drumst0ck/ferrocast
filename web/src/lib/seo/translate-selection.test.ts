import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { en, es } from './translate-selection';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('translate selected text pages', () => {
  it('have snippet-length titles and descriptions and the same structure in both languages', () => {
    for (const t of [en, es]) {
      expect(t.description.length).toBeGreaterThanOrEqual(120);
      expect(t.description.length).toBeLessThanOrEqual(155);
      expect(t.title.length).toBeLessThanOrEqual(70);
      for (const row of t.limits.rows) expect(row).toHaveLength(t.limits.cols.length);
    }
    expect(es.limits.rows).toHaveLength(en.limits.rows.length);
    expect(es.steps.items).toHaveLength(en.steps.items.length);
    expect(es.sections).toHaveLength(en.sections.length);
    expect(es.faq.items).toHaveLength(en.faq.items.length);
    expect(en.path).toBe('/translate-selected-text-mac/');
    expect(es.path).toBe('/es/traducir-texto-seleccionado-mac/');
  });

  it('states the defaults, limits and browsers the code defines', () => {
    const tr = read('crates/core/src/translate.rs');
    expect(tr).toContain('pub const MAX_LANGUAGES: usize = 12;');
    expect(tr).toContain('pub const MAX_CHARS: usize = 20_000;');
    expect(tr).toContain('["English", "Spanish", "French", "German", "Portuguese", "Italian", "Japanese", "Chinese"]');
    for (const b of ['com.apple.Safari', 'com.google.Chrome', 'com.brave.Browser', 'com.microsoft.edgemac', 'company.thebrowser.Browser', 'com.vivaldi.Vivaldi', 'com.operasoftware.Opera']) {
      expect(tr).toContain(`"${b}"`);
    }
    expect(tr).toContain('`None` for Firefox and everything else');
    expect(read('crates/core/src/models.rs')).toContain('wheel_translate_key: "Command".into()');
    expect(JSON.stringify(en)).toContain('Selections over 20,000 characters are refused');
    expect(JSON.stringify(es)).toContain('20 000 caracteres');
  });

  it('never logs or stores the text, as translate_log_line does', () => {
    const app = read('crates/app/src/app.rs');
    expect(app).toContain('"translate {lang} via {engine} ok chars={} ms={ms}"');
    expect(JSON.stringify(en)).toContain('never the text');
  });

  it('keeps macOS Translate as an honest rival: sourced, "Replace with Translation" named, no "cannot"', () => {
    expect(en.limits.note).toContain('support.apple.com/guide/mac-help/translate-text-on-mac-mchldd8b3c15');
    expect(es.limits.note).toContain('support.apple.com/es-es/guide/mac-help/mchldd8b3c15');
    expect(en.lead.join(' ')).toContain('Replace with Translation');
    expect(es.lead.join(' ')).toContain('Reemplazar por la traducción');
    const apple = JSON.stringify([en, es].map((t) => t.limits.rows.map((r) => r[2])));
    expect(apple).not.toMatch(/cannot|can’t|no puede|no permite|no tiene|carece/i);
  });

  it('says only Ollama, LM Studio and Apple keep the text on the Mac, and no speed claims', () => {
    for (const t of [en, es]) {
      const s = JSON.stringify(t);
      expect(s).toMatch(/Ollama/);
      expect(s).not.toMatch(/faster than|más rápid|\bRAM\b|100 ?%/i);
    }
    expect(read('web/src/content/docs/docs/launcher/ask-ai.md')).toContain('nothing leaves it unless you point it elsewhere');
  });

  it('avoids voseo', () => {
    expect(JSON.stringify(es)).not.toMatch(/(?<![\p{L}])(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)(?![\p{L}])/iu);
  });
});
