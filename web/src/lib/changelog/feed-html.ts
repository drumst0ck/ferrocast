import { CATEGORY_LABEL, CATEGORY_ORDER } from './load';
import type { ReleaseView } from './query';

const escapeHtml = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');

const titled = (e: { title: string | null; bodyHtml: string }) => `${e.title ? `<strong>${escapeHtml(e.title)}</strong>: ` : ''}${e.bodyHtml}`;

/** RSS `content:encoded` for one release: entries grouped by category. Titles are escaped; bodyHtml is already sanitized. */
export function rssFeedHtml(r: ReleaseView): string {
  return CATEGORY_ORDER.map((c) => {
    const es = r.entries.filter((e) => e.category === c);
    return es.length ? `<h3>${CATEGORY_LABEL[c]}</h3><ul>${es.map((e) => `<li>${titled(e)}</li>`).join('')}</ul>` : '';
  }).join('');
}

/** JSON Feed `content_html` for one release. */
export function jsonFeedHtml(r: ReleaseView): string {
  return r.entries.map((e) => `<p>${titled(e)}</p>`).join('');
}
