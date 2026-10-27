import { parseChangelog } from './parse';
import { readChangelogSource } from './read-source';

export type ChangelogSitemapEntry = { url: string; lastmod?: Date };

/** Released version URLs for @astrojs/sitemap (same CHANGELOG source as changelog:push). */
export function changelogSitemapEntries(site: string): ChangelogSitemapEntry[] {
  try {
    const source = readChangelogSource();
    if (!source) {
      console.warn('[sitemap] CHANGELOG.md not found under web/ or repo root; skipping changelog version URLs');
      return [];
    }
    const releases = parseChangelog(source);
    return releases
      .filter((r) => r.version !== 'unreleased' && r.releasedOn)
      .map((r) => ({
        url: new URL(`/changelog/${r.version}/`, site).href,
        lastmod: new Date(`${r.releasedOn}T00:00:00Z`),
      }));
  } catch (e) {
    console.warn('[sitemap] Failed to read or parse CHANGELOG.md:', e);
    return [];
  }
}
