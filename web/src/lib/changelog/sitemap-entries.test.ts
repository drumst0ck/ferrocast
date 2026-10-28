import { describe, expect, it } from 'vitest';
import { readChangelogSource } from './read-source';
import { changelogSitemapEntries } from './sitemap-entries';

describe('changelogSitemapEntries', () => {
  it('includes released versions with lastmod from web/CHANGELOG.md', () => {
    const entries = changelogSitemapEntries('https://ferrocast.app');
    // One entry per dated release heading, so the test does not need editing on every release.
    const released = (readChangelogSource() ?? '').match(/^## \[\d[^\]]*\] - \d{4}-\d{2}-\d{2}/gm) ?? [];
    expect(released.length).toBeGreaterThanOrEqual(6);
    expect(entries).toHaveLength(released.length);
    const v040 = entries.find((e) => e.url.endsWith('/changelog/0.4.0/'));
    expect(v040?.lastmod?.toISOString()).toBe('2026-10-28T00:00:00.000Z');
    const v030 = entries.find((e) => e.url.endsWith('/changelog/0.3.0/'));
    expect(v030?.lastmod?.toISOString()).toBe('2026-10-27T00:00:00.000Z');
    expect(entries.some((e) => e.url.includes('/unreleased/'))).toBe(false);
  });
});
