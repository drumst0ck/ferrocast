import rss from '@astrojs/rss';
import type { APIContext } from 'astro';
import { loadReleases, unavailableFeed, versionLabel } from '../../lib/changelog/load';
import { rssFeedHtml } from '../../lib/changelog/feed-html';

export const prerender = false;

export async function GET(context: APIContext) {
  const releases = await loadReleases();
  if (!releases) return unavailableFeed();
  const items = releases
    .filter((r) => r.releasedOn)
    .map((r) => ({
      title: `Ferrocast ${versionLabel(r.version)}`,
      pubDate: new Date(r.releasedOn as string),
      link: `/changelog/${r.version}/`,
      description: r.summary ?? `Changes in Ferrocast ${r.version}`,
      content: rssFeedHtml(r),
    }));
  const res = await rss({ title: 'Ferrocast changelog', description: 'Release notes for Ferrocast, the fast launcher for macOS.', site: context.site!, items });
  res.headers.set('Cache-Control', 'public, max-age=300, stale-while-revalidate=3600');
  return res;
}
