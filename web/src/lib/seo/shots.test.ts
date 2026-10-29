import { describe, expect, it } from 'vitest';
import { existsSync } from 'node:fs';
import sharp from 'sharp';
import { PAGES_WITHOUT_SHOT, SHOTS, TABLE_PAGES, allShotPaths, shotsFor } from './shots';

describe('guide screenshots (brief §1)', () => {
  it.each(TABLE_PAGES)('%s has 1-3 existing /img/ screenshots with a non-empty alt', (path) => {
    const shots = shotsFor(path);
    expect(shots.length).toBeGreaterThanOrEqual(1);
    expect(shots.length).toBeLessThanOrEqual(3);
    for (const s of shots) {
      expect(s.src.startsWith('/img/')).toBe(true);
      expect(existsSync(`public${s.src}`)).toBe(true);
      expect(s.alt.trim().length).toBeGreaterThan(10);
    }
  });

  it('Spanish pages get Spanish alts that differ from the English ones', () => {
    for (const path of TABLE_PAGES.filter((p) => p.startsWith('/es/'))) {
      for (const s of shotsFor(path)) {
        expect(s.alt).not.toBe(SHOTS[s.file].en);
        expect(s.alt).toBe(SHOTS[s.file].es);
      }
    }
  });

  it('width and height are the real pixel size of each file', async () => {
    for (const [file, shot] of Object.entries(SHOTS)) {
      const m = await sharp(`public/img/${file}.jpg`).metadata();
      expect([shot.w, shot.h], file).toEqual([m.width, m.height]);
    }
  });

  it('pages with no suitable screenshot get none and are listed', () => {
    expect(PAGES_WITHOUT_SHOT.length).toBeGreaterThan(0);
    for (const p of PAGES_WITHOUT_SHOT) expect(shotsFor(p)).toEqual([]);
    expect(PAGES_WITHOUT_SHOT).toContain('/mac-window-management/');
    expect(PAGES_WITHOUT_SHOT).toContain('/raycast-script-commands/');
  });

  it('never maps the same path twice across the table and the no-shot list', () => {
    const all = [...TABLE_PAGES, ...PAGES_WITHOUT_SHOT];
    expect(new Set(all).size).toBe(all.length);
    expect(allShotPaths().length).toBeGreaterThan(20);
  });
});
