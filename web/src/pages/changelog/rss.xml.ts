import rss from '@astrojs/rss';
import type { APIContext } from 'astro';
import { loadReleases, CATEGORY_LABEL, CATEGORY_ORDER, versionLabel } from '../../lib/changelog/load';

export const prerender = false;

export async function GET(context: APIContext) {
  const releases = (await loadReleases()) ?? [];
  const items = releases
    .filter((r) => r.releasedOn)
    .map((r) => ({
      title: `Ferrocast ${versionLabel(r.version)}`,
      pubDate: new Date(r.releasedOn as string),
      link: `/changelog/${r.version}/`,
      description: r.summary ?? `Changes in Ferrocast ${r.version}`,
      content: CATEGORY_ORDER.map((c) => {
        const es = r.entries.filter((e) => e.category === c);
        return es.length ? `<h3>${CATEGORY_LABEL[c]}</h3><ul>${es.map((e) => `<li>${e.title ? `<strong>${e.title}</strong>: ` : ''}${e.bodyHtml}</li>`).join('')}</ul>` : '';
      }).join(''),
    }));
  const res = await rss({ title: 'Ferrocast changelog', description: 'Release notes for Ferrocast, the fast launcher for macOS.', site: context.site!, items });
  res.headers.set('Cache-Control', 'public, max-age=300, stale-while-revalidate=3600');
  return res;
}
