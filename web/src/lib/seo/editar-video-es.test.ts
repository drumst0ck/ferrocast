import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { page } from './editar-video-es';
import { allTools } from './tools';

const root = new URL('../../../../', import.meta.url);
const read = (p: string) => readFileSync(new URL(p, root), 'utf8');

describe('video editing page (ES)', () => {
  it('has a description of snippet length and a cell for every column', () => {
    expect(page.description.length).toBeGreaterThanOrEqual(120);
    expect(page.description.length).toBeLessThanOrEqual(160);
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
  });

  it('says ffmpeg is required with the brew command', () => {
    expect(JSON.stringify(page)).toContain('brew install ffmpeg');
    expect(read('web/src/content/docs/docs/files/optional-tools.md')).toContain('`brew install ffmpeg`');
  });

  it('matches the ffmpeg behaviour in media.rs and the shortcuts in tools.json', () => {
    const media = read('crates/core/src/media.rs');
    expect(media).toContain('factor.clamp(0.25, 4.0)');
    expect(media).toContain('"-c", "copy"');
    expect(media).toContain('"-c:v", "copy", "-an"');
    expect(media).toContain('"-c:a", "copy"');
    const tools = allTools();
    const key = (id: string) => tools.find((t) => t.id === id)?.shortcut;
    expect(key('trimVideo')).toBe('⇧⌘V');
    expect(key('changeVideoSpeed')).toBe('⇧⌘L');
    expect(key('cropVideo')).toBe('⇧⌘J');
    expect(key('muteVideo')).toBeNull();
  });

  it('uses the output suffixes and typed commands the app has', () => {
    const conv = read('crates/core/src/convert.rs');
    for (const s of ['"trimmed"', '"muted"', '"cropped"', '"joined"', '"part"']) expect(conv).toContain(s);
    expect(read('crates/app/src/services.rs')).toContain('"-segment_time"');
    expect(read('web/src/content/docs/docs/files/tools/trim-video.mdx')).toContain('`trim 0:10-0:30`');
    expect(read('web/src/content/docs/docs/files/tools/video-speed.mdx')).toContain('`speed 1.5x`');
    expect(read('web/src/content/docs/docs/files/tools/crop-video.mdx')).toContain('`crop 640x360 at 100,50`');
  });

  it('never promises quality or says "exact size", and never uses voseo', () => {
    const s = JSON.stringify(page);
    expect(s).not.toMatch(/sin perder calidad|mejor que iMovie|más rápid/i);
    expect(s).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });

  it('links only to pages that exist', () => {
    const slugs = allTools().map((t) => t.slug);
    for (const [, href] of page.more.links) {
      const m = href.match(/^\/tools\/([^/]+)\/$/);
      if (m) expect(slugs).toContain(m[1]);
    }
  });
});
