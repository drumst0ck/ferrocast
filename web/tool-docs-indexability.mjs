import manifest from './src/data/tool-docs-indexability.json' with { type: 'json' };
const entries = new Map(manifest.entries.map((e) => [e.path, e]));
/** The manifest entry for a /docs/files/tools/<slug>/ URL or path, or undefined (also when the manifest is disabled). */
export function toolDocEntry(url) {
  if (!manifest.enabled) return undefined;
  const path = new URL(url, 'https://ferrocast.app').pathname.replace(/^\/es(?=\/docs\/)/, '');
  return entries.get(path);
}
/** Kept out of the sitemap whenever the English doc has an entry (both decisions). */
export function isToolDocExcluded(url) {
  return !new URL(url, 'https://ferrocast.app').pathname.startsWith('/es/') && toolDocEntry(url) !== undefined;
}
