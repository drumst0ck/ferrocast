import { describe, expect, it } from 'vitest';
import { changelogHref, isSafeDocsPath, parsePageParam } from './links';
import { parseChangelog } from './parse';

describe('parsePageParam', () => {
  it('accepts plain positive integers only', () => {
    expect(parsePageParam('3')).toBe(3);
    expect(parsePageParam(null)).toBe(1);
    expect(parsePageParam('abc')).toBe(1);
    expect(parsePageParam('0')).toBe(1);
    expect(parsePageParam('-2')).toBe(1);
    expect(parsePageParam('2.5')).toBe(1);
  });
  it('rejects values that overflow the SQL offset', () => {
    expect(parsePageParam('1e20')).toBe(1);
    expect(parsePageParam('99999999999999999999')).toBe(1);
    expect(parsePageParam('100000')).toBe(1);
  });
});

describe('changelogHref', () => {
  it('keeps the tag filter when paginating and resets the page when picking a tag', () => {
    expect(changelogHref(1, null)).toBe('/changelog/');
    expect(changelogHref(2, null)).toBe('/changelog/?page=2');
    expect(changelogHref(1, 'files')).toBe('/changelog/?tag=files');
    expect(changelogHref(3, 'files')).toBe('/changelog/?tag=files&page=3');
  });
});

describe('docs link safety', () => {
  it('only allows same-site absolute paths', () => {
    expect(isSafeDocsPath('/docs/x/')).toBe(true);
    expect(isSafeDocsPath('//evil.example/x')).toBe(false);
    expect(isSafeDocsPath('/\\evil.example')).toBe(false);
    expect(isSafeDocsPath('https://evil.example')).toBe(false);
    expect(isSafeDocsPath(null)).toBe(false);
  });
  it('parser does not take a protocol-relative docs link', () => {
    const md = '# Changelog\n\n## [Unreleased]\n\n### Added\n- **A**: body (docs: //evil.example/x)\n';
    const e = parseChangelog(md)[0].entries[0];
    expect(e.docsUrl).toBeNull();
  });
});
