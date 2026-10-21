import type { APIContext } from 'astro';
import { loadReleases, versionLabel } from '../lib/changelog/load';

export const prerender = false;

// JSON Feed 1.1: https://www.jsonfeed.org/version/1.1/
export async function GET(context: APIContext) {
  const releases = (await loadReleases()) ?? [];
  const site = context.site!.href.replace(/\/$/, '');
  const feed = {
    version: 'https://jsonfeed.org/version/1.1',
    title: 'Ferrocast changelog',
    home_page_url: `${site}/changelog/`,
    feed_url: `${site}/changelog.json`,
    items: releases
      .filter((r) => r.releasedOn)
      .map((r) => ({
        id: `${site}/changelog/${r.version}/`,
        url: `${site}/changelog/${r.version}/`,
        title: `Ferrocast ${versionLabel(r.version)}`,
        date_published: new Date(r.releasedOn as string).toISOString(),
        content_html: r.entries.map((e) => `<p>${e.title ? `<strong>${e.title}</strong>: ` : ''}${e.bodyHtml}</p>`).join(''),
        tags: [...new Set(r.entries.flatMap((e) => e.tags))],
      })),
  };
  return new Response(JSON.stringify(feed), { headers: { 'Content-Type': 'application/feed+json', 'Cache-Control': 'public, max-age=300, stale-while-revalidate=3600' } });
}
