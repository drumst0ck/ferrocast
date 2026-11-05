import { readdirSync, readFileSync, statSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(__dirname, '../content/docs');
const es = join(root, 'es/docs');
const en = join(root, 'docs');

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}
const headings = (s: string) => s.split('\n').filter((l) => /^#{1,6} /.test(l)).length;
const kbds = (s: string) => (s.match(/<kbd>/g) ?? []).length;
const twins = walk(es)
  .filter((p) => /\.mdx?$/.test(p))
  .map((p) => relative(es, p))
  .filter((rel) => existsSync(join(en, rel)));

describe('Spanish docs keep up with the English pages', () => {
  it('finds twins', () => expect(twins.length).toBeGreaterThan(5));
  for (const rel of twins) {
    it(`${rel}: same headings and key chips`, () => {
      const e = readFileSync(join(en, rel), 'utf8');
      const s = readFileSync(join(es, rel), 'utf8');
      expect({ rel, headings: headings(s), kbd: kbds(s) }).toEqual({ rel, headings: headings(e), kbd: kbds(e) });
    });
  }
  it('drag-wheel documents the mouse path (chips under the ring)', () => {
    expect(readFileSync(join(es, 'files/drag-wheel.md'), 'utf8')).toMatch(/chip/i);
  });
});
