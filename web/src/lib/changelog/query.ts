import { asc, desc, eq, inArray, sql } from 'drizzle-orm';
import type { Db } from '../../db/client';
import { entries, entryTags, releases, tags } from '../../db/schema';
import type { Category } from './parse';

export interface EntryView {
  id: number;
  category: Category;
  slug: string;
  title: string | null;
  bodyMd: string;
  bodyHtml: string;
  docsUrl: string | null;
  position: number;
  updatedAt: Date;
  tags: string[];
}
export interface ReleaseView {
  id: number;
  version: string;
  releasedOn: string | null;
  summary: string | null;
  entries: EntryView[];
}

async function hydrate(db: Db, rels: { id: number; version: string; releasedOn: string | null; summary: string | null }[]): Promise<ReleaseView[]> {
  if (!rels.length) return [];
  const rows = await db.select().from(entries).where(inArray(entries.releaseId, rels.map((r) => r.id))).orderBy(asc(entries.position));
  const tagRows = rows.length
    ? await db.select({ entryId: entryTags.entryId, name: tags.name }).from(entryTags).innerJoin(tags, eq(tags.id, entryTags.tagId)).where(inArray(entryTags.entryId, rows.map((e) => e.id)))
    : [];
  const tagMap = new Map<number, string[]>();
  for (const t of tagRows) tagMap.set(t.entryId, [...(tagMap.get(t.entryId) ?? []), t.name].sort());
  return rels.map((r) => ({
    ...r,
    entries: rows.filter((e) => e.releaseId === r.id).map((e) => ({ id: e.id, category: e.category, slug: e.slug, title: e.title, bodyMd: e.bodyMd, bodyHtml: e.bodyHtml, docsUrl: e.docsUrl, position: e.position, updatedAt: e.updatedAt, tags: tagMap.get(e.id) ?? [] })),
  }));
}

export const CHANGELOG_PAGE_SIZE = 50;

const newestFirst = [sql`${releases.releasedOn} desc nulls first`, desc(releases.id)] as const;

/** Every release, unreleased first, then newest release date first (feeds and the JSON API). */
export async function listReleases(db: Db): Promise<ReleaseView[]> {
  const rels = await db.select().from(releases).orderBy(...newestFirst);
  return hydrate(db, rels);
}

/** One page of the changelog index; `hasMore` is true when older releases exist. Pages start at 1. */
export async function listReleasesPage(db: Db, page: number, size = CHANGELOG_PAGE_SIZE): Promise<{ releases: ReleaseView[]; hasMore: boolean }> {
  const n = Number.isInteger(page) && page > 0 ? page : 1;
  const rels = await db.select().from(releases).orderBy(...newestFirst).limit(size + 1).offset((n - 1) * size);
  return { releases: await hydrate(db, rels.slice(0, size)), hasMore: rels.length > size };
}

export async function getRelease(db: Db, version: string): Promise<ReleaseView | null> {
  const [rel] = await db.select().from(releases).where(eq(releases.version, version));
  return rel ? (await hydrate(db, [rel]))[0] : null;
}
