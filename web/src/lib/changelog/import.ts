import { and, eq, inArray, notInArray } from 'drizzle-orm';
import type { Db } from '../../db/client';
import { entries, entryTags, imports, releases, tags } from '../../db/schema';
import { renderBody } from './render';
import type { ImportPayload } from './validate';

export interface ImportResult {
  releases: number;
  entries: number;
  created: number;
  updated: number;
  deleted: number;
}

/** Makes the database equal to the payload. Transactional and idempotent: untouched rows keep updated_at. */
export async function importChangelog(db: Db, payload: ImportPayload): Promise<ImportResult> {
  const res: ImportResult = { releases: payload.releases.length, entries: 0, created: 0, updated: 0, deleted: 0 };
  await db.transaction(async (tx) => {
    const versions = payload.releases.map((r) => r.version);
    const gone = versions.length ? await tx.delete(releases).where(notInArray(releases.version, versions)).returning({ id: releases.id }) : await tx.delete(releases).returning({ id: releases.id });
    res.deleted += gone.length;

    for (const rel of payload.releases) {
      const [existing] = await tx.select().from(releases).where(eq(releases.version, rel.version));
      let releaseId: number;
      if (!existing) {
        [{ id: releaseId }] = await tx.insert(releases).values({ version: rel.version, releasedOn: rel.releasedOn, summary: rel.summary }).returning({ id: releases.id });
      } else {
        releaseId = existing.id;
        if (existing.releasedOn !== rel.releasedOn || existing.summary !== rel.summary) {
          await tx.update(releases).set({ releasedOn: rel.releasedOn, summary: rel.summary, updatedAt: new Date() }).where(eq(releases.id, releaseId));
        }
      }

      const current = await tx.select().from(entries).where(eq(entries.releaseId, releaseId));
      const bySlug = new Map(current.map((e) => [e.slug, e]));
      const keep = rel.entries.map((e) => e.slug);
      const stale = current.filter((e) => !keep.includes(e.slug)).map((e) => e.id);
      if (stale.length) {
        await tx.delete(entries).where(inArray(entries.id, stale));
        res.deleted += stale.length;
      }

      for (const e of rel.entries) {
        res.entries++;
        const row = { category: e.category, title: e.title, bodyMd: e.bodyMd, bodyHtml: renderBody(e.bodyMd), docsUrl: e.docsUrl, position: e.position };
        const old = bySlug.get(e.slug);
        let entryId: number;
        if (!old) {
          [{ id: entryId }] = await tx.insert(entries).values({ releaseId, slug: e.slug, ...row }).returning({ id: entries.id });
          res.created++;
        } else {
          entryId = old.id;
          const changed = old.category !== row.category || old.title !== row.title || old.bodyMd !== row.bodyMd || old.docsUrl !== row.docsUrl || old.position !== row.position;
          if (changed) {
            await tx.update(entries).set({ ...row, updatedAt: new Date() }).where(eq(entries.id, entryId));
            res.updated++;
          }
        }
        await syncTags(tx, entryId, e.tags);
      }
    }
    await tx.insert(imports).values({ sourceSha: payload.sourceSha, entriesCount: res.entries });
  });
  return res;
}

async function syncTags(tx: Parameters<Parameters<Db['transaction']>[0]>[0], entryId: number, names: string[]) {
  const have = await tx.select({ name: tags.name, id: tags.id }).from(entryTags).innerJoin(tags, eq(tags.id, entryTags.tagId)).where(eq(entryTags.entryId, entryId));
  const haveNames = have.map((t) => t.name).sort().join(',');
  if (haveNames === [...names].sort().join(',')) return;
  await tx.delete(entryTags).where(eq(entryTags.entryId, entryId));
  for (const name of names) {
    await tx.insert(tags).values({ name }).onConflictDoNothing();
    const [t] = await tx.select().from(tags).where(eq(tags.name, name));
    await tx.insert(entryTags).values({ entryId, tagId: t.id }).onConflictDoNothing();
  }
}
void and;
