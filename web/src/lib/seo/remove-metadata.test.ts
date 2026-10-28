import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { en, es } from './remove-metadata';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('remove metadata pages', () => {
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
    expect(en.path).toBe('/remove-metadata-mac/');
    expect(es.path).toBe('/es/quitar-metadatos-mac/');
  });

  it('describes photo stripping as the code does: JPEG and PNG only, image data copied, copy named clean', () => {
    const convert = read('crates/core/src/convert.rs');
    expect(convert).toContain('drops APP1–APP15 (EXIF, XMP, IPTC, ICC…) and comments');
    expect(convert).toContain('the rest is image data, copy as is');
    expect(convert).toMatch(/b"eXIf" \| b"tEXt" \| b"iTXt" \| b"zTXt" \| b"tIME"/);
    const services = read('crates/app/src/services.rs');
    expect(services).toContain('metadata removal supports JPEG and PNG; convert to one of those first');
    expect(services).toContain('"jpg" | "jpeg" => strip_jpeg(&bytes)');
    expect(services).toContain('"png" => strip_png(&bytes)');
    expect(read('crates/core/src/convert.rs')).toContain('ImageOp::StripMetadata => "clean".into()');
    expect(JSON.stringify(en)).toContain('metadata removal supports JPEG and PNG; convert to one of those first');
  });

  it('describes audio, video, GIF and PDF as the code does', () => {
    const meta = read('crates/core/src/metaform.rs');
    expect(meta).toContain('"-map_metadata", "-1", "-map_chapters", "-1"');
    expect(meta).toContain('"-map", "0", "-c", "copy"');
    const pdf = read('crates/macos/src/pdf.rs');
    expect(pdf).toContain("['Title', 'Author', 'Subject', 'Keywords', 'Creator', 'Producer']");
    expect(pdf).toContain('"<name> edited.pdf"');
    const draw = read('crates/app/src/draw.rs');
    expect(draw).toContain('Container brands and some muxer tags stay');
    expect(draw).toContain('Creator and Producer are shown but not written back.');
    expect(draw).toContain('GIF files cannot store tags: you can only remove all metadata.');
  });

  it('keeps the honest limits in both languages', () => {
    for (const t of [en, es]) {
      const s = JSON.stringify(t);
      expect(s).toContain('HEIC');
      expect(s).toMatch(/Creator/);
      expect(s).not.toMatch(/\b(untraceable|no traces|sin dejar rastro|100 ?%)/i);
      expect(s).not.toMatch(/faster than|más rápid|\bRAM\b/i);
    }
  });

  it('never says Apple apps cannot do something and never uses voseo', () => {
    const apple = JSON.stringify([en, es].map((t) => t.limits.rows.map((r) => r.slice(2))));
    expect(apple).not.toMatch(/cannot|can’t|no puede|no permite|no tiene/i);
    expect(JSON.stringify(es)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
