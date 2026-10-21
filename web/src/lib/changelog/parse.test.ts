import { describe, expect, it } from 'vitest';
import { parseChangelog } from './parse';

const ok = `# Changelog

Intro text ignored.

## [Unreleased]

### Added
- **Compress**: shrink images to a size. #files #images (docs: /docs/files/tools/compress/)
- Plain entry without title.
  continued on a second line.

### Fixed
- Calculator: \`200 * 10%\` works.

## [0.1.0] - 2026-10-15

First public build.

### Added
- **Search**: apps and files.
`;

describe('parseChangelog', () => {
  const r = parseChangelog(ok);
  it('reads releases in order with dates', () => {
    expect(r.map((x) => [x.version, x.releasedOn])).toEqual([['unreleased', null], ['0.1.0', '2026-10-15']]);
  });
  it('reads the summary paragraph of a release', () => {
    expect(r[1].summary).toBe('First public build.');
    expect(r[0].summary).toBeNull();
  });
  it('extracts title, tags and docs link, and strips them from the body', () => {
    const e = r[0].entries[0];
    expect(e).toMatchObject({ category: 'added', title: 'Compress', docsUrl: '/docs/files/tools/compress/', tags: ['files', 'images'], position: 0 });
    expect(e.bodyMd).toBe('shrink images to a size.');
  });
  it('joins continuation lines and keeps untitled entries untitled', () => {
    const e = r[0].entries[1];
    expect(e.title).toBeNull();
    expect(e.bodyMd).toBe('Plain entry without title. continued on a second line.');
  });
  it('maps categories and does not treat "Word:" as a title', () => {
    const f = r[0].entries[2];
    expect(f.category).toBe('fixed');
    expect(f.title).toBeNull();
  });
  it('gives stable unique slugs', () => {
    const again = parseChangelog(ok);
    expect(again[0].entries.map((e) => e.slug)).toEqual(r[0].entries.map((e) => e.slug));
    expect(new Set(r.flatMap((x) => x.entries.map((e) => e.slug))).size).toBe(4);
  });
  it('rejects a bullet outside a category with its line number', () => {
    expect(() => parseChangelog('## [1.0.0] - 2026-01-01\n- stray\n')).toThrow('line 2: entry outside a ### category');
  });
  it('rejects an unknown category', () => {
    expect(() => parseChangelog('## [1.0.0] - 2026-01-01\n### Misc\n')).toThrow('line 2: unknown category "Misc"');
  });
  it('rejects a duplicate version', () => {
    expect(() => parseChangelog('## [1.0.0] - 2026-01-01\n## [1.0.0] - 2026-01-02\n')).toThrow('line 2: duplicate version 1.0.0');
  });
  it('rejects a malformed release header', () => {
    expect(() => parseChangelog('## 1.0.0 soon\n')).toThrow('line 1: malformed release header');
  });
});
