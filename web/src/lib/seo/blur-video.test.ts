import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { plain } from './markup';
import { en, es } from './blur-video';

const words = (s: string) => plain(s).split(/\s+/).filter(Boolean).length;
const body = (t: typeof en) =>
  words(
    [
      ...t.lead,
      ...t.before.yes,
      ...t.before.no,
      ...t.steps.items.flat(),
      ...t.command.paras,
      ...t.moving.paras,
      ...t.photos.paras,
      ...t.table.rows.flat(),
      t.table.note,
      ...t.faq.items.flat(),
      t.closing,
    ].join(' '),
  );

describe('blur video pages', () => {
  it('uses the brief URLs, titles, metas and H1s', () => {
    expect(en.path).toBe('/blur-video-mac/');
    expect(es.path).toBe('/es/difuminar-video-mac/');
    expect(en.title).toBe('Blur Part of a Video on Mac, Free and Offline | Ferrocast');
    expect(es.title).toBe('Difuminar parte de un vídeo en Mac, gratis y sin subirlo');
    expect(en.h1).toBe('Blur part of a video on your Mac, without uploading it');
    expect(es.h1).toBe('Difuminar parte de un vídeo en Mac sin subirlo a internet');
    expect(en.description).toBe(
      'Cover up to 64 areas of a video on your Mac, each with its own start and end time, with blur or a solid box. Free, local, needs ffmpeg. No face tracking.',
    );
    expect(es.description).toBe(
      'Tapa hasta 64 zonas de un vídeo en tu Mac, cada una con su inicio y su fin, con desenfoque o un bloque de color. Gratis, local, necesita ffmpeg. Sin seguimiento de caras.',
    );
    expect(en.description.length).toBeGreaterThanOrEqual(140);
    expect(en.description.length).toBeLessThanOrEqual(160);
    // Brief Meta A for Spanish is 170 characters. It is the prescribed snippet, so the usual 160 cap is not applied.
    expect(es.description.length).toBe(170);
  });

  it('keeps both languages in the brief length and the same shape', () => {
    for (const t of [en, es]) {
      const n = body(t);
      expect(n).toBeGreaterThanOrEqual(900);
      expect(n).toBeLessThanOrEqual(1300);
    }
    expect(es.steps.items).toHaveLength(en.steps.items.length);
    expect(es.faq.items).toHaveLength(en.faq.items.length);
    expect(es.table.rows).toHaveLength(en.table.rows.length);
    expect(es.before.yes).toHaveLength(en.before.yes.length);
    expect(es.before.no).toHaveLength(en.before.no.length);
  });

  it('states the citable limits and does not claim video face tracking', () => {
    expect(en.faq.items[0][1]).toMatch(/does not track moving faces/);
    expect(es.faq.items[1][1]).toMatch(/La detección automática de caras y texto solo funciona en fotos/);
    expect(es.faq.items[2][1]).toMatch(/guarda una copia nueva junto al original/);
    const claims = JSON.stringify([en, es]);
    expect(claims).not.toMatch(/detects faces in (a )?video|detecta caras en (un |el )?vídeo/i);
    expect(claims).not.toMatch(/\$20|\$192|\$59|20 \$\/mes|192 \$|desde 59/);
    expect(claims).not.toMatch(/QuickTime (cannot|can't|does not) blur|QuickTime no (puede |difumina)/i);
    expect(claims).not.toMatch(/iCloud|Clear History|coming in the next version|próxima versión/i);
  });

  it('never uses voseo in the Spanish copy', () => {
    expect(JSON.stringify(es)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí|escribí|instalá)\b/i);
  });

  it('marks the app as a free macOS multimedia application in the page', () => {
    const page = readFileSync('src/components/seo/BlurVideoPage.astro', 'utf8');
    expect(page).toContain("applicationCategory: 'MultimediaApplication'");
    expect(page).toContain("operatingSystem: 'macOS 13+'");
    expect(page).toContain("price: '0'");
  });
});
