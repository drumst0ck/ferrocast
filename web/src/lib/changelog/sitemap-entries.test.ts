import { describe, expect, it } from 'vitest';
import { changelogSitemapEntries } from './sitemap-entries';

describe('changelogSitemapEntries', () => {
  it('includes released versions with lastmod from web/CHANGELOG.md', () => {
    const entries = changelogSitemapEntries('https://ferrocast.app');
    expect(entries).toHaveLength(6);
    const v030 = entries.find((e) => e.url.endsWith('/changelog/0.3.0/'));
    expect(v030?.lastmod?.toISOString()).toBe('2026-10-27T00:00:00.000Z');
    expect(entries.some((e) => e.url.includes('/unreleased/'))).toBe(false);
  });
});
