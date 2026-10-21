import type { APIRoute } from 'astro';
import { loadReleases } from '../../../lib/changelog/load';

export const prerender = false;

export const GET: APIRoute = async () => {
  const releases = await loadReleases();
  if (!releases) return new Response(JSON.stringify({ error: 'changelog unavailable' }), { status: 503, headers: { 'Content-Type': 'application/json' } });
  const body = releases.map((r) => ({
    version: r.version,
    releasedOn: r.releasedOn,
    summary: r.summary,
    entries: r.entries.map((e) => ({ category: e.category, title: e.title, body: e.bodyMd, html: e.bodyHtml, docsUrl: e.docsUrl, tags: e.tags })),
  }));
  return new Response(JSON.stringify({ releases: body }), {
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'public, max-age=300, stale-while-revalidate=3600' },
  });
};
