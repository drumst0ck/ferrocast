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

describe('parseChangelog continuation and stray text', () => {
  const wrap = (body: string) => `## [Unreleased]\n\n### Added\n${body}\n`;
  it('appends an indented paragraph after a blank line to the entry', () => {
    const [r] = parseChangelog(wrap('- **A**: first\n\n  continued here\n\n- **B**: second'));
    expect(r.entries.map((e) => e.bodyMd)).toEqual(['first continued here', 'second']);
    expect(r.entries[0].title).toBe('A');
  });
  it('rejects prose that is neither an entry nor a continuation', () => {
    expect(() => parseChangelog(wrap('- **A**: first\n\nStray prose'))).toThrow(/line 6: text outside an entry/);
    expect(() => parseChangelog(wrap('Stray prose\n- **A**: first'))).toThrow(/line 4: text outside an entry/);
  });
  it('still accepts blank lines between entries and before the next category', () => {
    const [r] = parseChangelog(wrap('- **A**: first\n\n- **B**: second\n\n### Fixed\n- **C**: third'));
    expect(r.entries.map((e) => e.bodyMd)).toEqual(['first', 'second', 'third']);
  });
});
