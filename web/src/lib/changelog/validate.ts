import type { Category, ParsedRelease } from './parse';

export interface ImportPayload {
  sourceSha: string;
  releases: ParsedRelease[];
}

const CATS: Category[] = ['added', 'changed', 'fixed', 'removed', 'security', 'limitation'];
const isStr = (v: unknown): v is string => typeof v === 'string';

/** Returns the payload typed, or throws Error with a message naming the offending field. */
export function validatePayload(input: unknown): ImportPayload {
  const o = input as Record<string, unknown> | null;
  if (!o || typeof o !== 'object') throw new Error('body must be a JSON object');
  if (!isStr(o.sourceSha) || !o.sourceSha) throw new Error('sourceSha must be a non-empty string');
  if (!Array.isArray(o.releases)) throw new Error('releases must be an array');
  const versions = new Set<string>();
  o.releases.forEach((r: any, i: number) => {
    const at = `releases[${i}]`;
    if (!r || !isStr(r.version) || !r.version) throw new Error(`${at}.version must be a non-empty string`);
    if (versions.has(r.version)) throw new Error(`${at}.version ${r.version} is duplicated`);
    versions.add(r.version);
    if (r.releasedOn !== null && !(isStr(r.releasedOn) && /^\d{4}-\d{2}-\d{2}$/.test(r.releasedOn))) throw new Error(`${at}.releasedOn must be YYYY-MM-DD or null`);
    if (r.summary != null && !isStr(r.summary)) throw new Error(`${at}.summary must be a string or null`);
    if (!Array.isArray(r.entries)) throw new Error(`${at}.entries must be an array`);
    const slugs = new Set<string>();
    r.entries.forEach((e: any, j: number) => {
      const ea = `${at}.entries[${j}]`;
      if (!e || !CATS.includes(e.category)) throw new Error(`${ea}.category must be one of ${CATS.join(', ')}`);
      if (!isStr(e.slug) || !e.slug) throw new Error(`${ea}.slug must be a non-empty string`);
      if (slugs.has(e.slug)) throw new Error(`${ea}.slug ${e.slug} is duplicated`);
      slugs.add(e.slug);
      if (!isStr(e.bodyMd) || !e.bodyMd) throw new Error(`${ea}.bodyMd must be a non-empty string`);
      if (e.title !== null && !isStr(e.title)) throw new Error(`${ea}.title must be a string or null`);
      if (e.docsUrl !== null && !(isStr(e.docsUrl) && e.docsUrl.startsWith('/'))) throw new Error(`${ea}.docsUrl must be a path starting with / or null`);
      if (!Array.isArray(e.tags) || !e.tags.every((t: unknown) => isStr(t) && /^[a-z0-9-]+$/.test(t))) throw new Error(`${ea}.tags must be lowercase words`);
      if (!Number.isInteger(e.position)) throw new Error(`${ea}.position must be an integer`);
    });
  });
  return o as unknown as ImportPayload;
}
