import { describe, expect, it } from 'vitest';
import type { Db } from '../../db/client';
import { CHANGELOG_PAGE_SIZE, listReleases, listReleasesPage } from './query';

/** Fake query builder: records limit/offset and answers the releases select with `total` rows. */
function fakeDb(total: number) {
  const seen: { limit?: number; offset?: number } = {};
  const all = Array.from({ length: total }, (_, i) => ({ id: i + 1, version: `0.${i}.0`, releasedOn: null, summary: null }));
  const db = {
    select: () => {
      const chain: Record<string, unknown> = {};
      chain.from = () => chain;
      chain.where = () => chain;
      chain.innerJoin = () => chain;
      chain.orderBy = () => chain;
      chain.limit = (n: number) => ((seen.limit = n), chain);
      chain.offset = (n: number) => ((seen.offset = n), chain);
      chain.then = (ok: (v: unknown) => unknown) => {
        // The releases select is the only one that sets a limit/offset; hydrate() selects return nothing.
        const rows = seen.limit === undefined ? [] : all.slice(seen.offset ?? 0, (seen.offset ?? 0) + seen.limit);
        return Promise.resolve(rows).then(ok);
      };
      return chain;
    },
  } as unknown as Db;
  return { db, seen };
}

describe('changelog pagination', () => {
  it('pages past 50 releases and reports whether older ones exist', async () => {
    const f = fakeDb(120);
    const p1 = await listReleasesPage(f.db, 1);
    expect(f.seen).toEqual({ limit: CHANGELOG_PAGE_SIZE + 1, offset: 0 });
    expect(p1.releases).toHaveLength(CHANGELOG_PAGE_SIZE);
    expect(p1.hasMore).toBe(true);
    const f3 = fakeDb(120);
    const p3 = await listReleasesPage(f3.db, 3);
    expect(f3.seen.offset).toBe(2 * CHANGELOG_PAGE_SIZE);
    expect(p3.releases).toHaveLength(20);
    expect(p3.hasMore).toBe(false);
  });
  it('the last full page has no older link', async () => {
    const p = await listReleasesPage(fakeDb(CHANGELOG_PAGE_SIZE).db, 1);
    expect(p.hasMore).toBe(false);
  });
  it('feeds take every release: no hidden default cap', async () => {
    const f = fakeDb(120);
    await listReleases(f.db);
    expect(f.seen.limit).toBeUndefined();
  });
});
