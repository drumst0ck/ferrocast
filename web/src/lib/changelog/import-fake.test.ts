import { getTableName } from 'drizzle-orm';
import { describe, expect, it } from 'vitest';
import type { Db } from '../../db/client';
import { importChangelog } from './import';
import { parseChangelog } from './parse';

/**
 * Runs importChangelog without Postgres (the real-database suite in import.test.ts is skipped
 * unless TEST_DATABASE_URL is set). The fake records every statement and answers selects and
 * returning() clauses from a script keyed by "<op>:<table>".
 */
interface Call { op: string; table: string; where: boolean; values?: unknown; set?: unknown; conflict?: boolean }

function fakeDb(script: Record<string, unknown[][]> = {}) {
  const calls: Call[] = [];
  const queues = new Map(Object.entries(script).map(([k, v]) => [k, [...v]]));
  let transactions = 0;
  let failOn: string | null = null;
  const statement = (op: string, table: unknown) => {
    const call: Call = { op, table: getTableName(table as never), where: false };
    calls.push(call);
    const key = `${op}:${call.table}`;
    const chain: Record<string, unknown> = {};
    const self = () => chain;
    for (const m of ['from', 'innerJoin', 'orderBy', 'limit']) chain[m] = self;
    chain.where = () => ((call.where = true), chain);
    chain.values = (v: unknown) => ((call.values = v), chain);
    chain.set = (v: unknown) => ((call.set = v), chain);
    chain.onConflictDoNothing = () => ((call.conflict = true), chain);
    chain.returning = self;
    chain.then = (ok: (v: unknown) => unknown, err: (e: unknown) => unknown) => {
      if (failOn === key) return Promise.reject(new Error(`boom ${key}`)).then(ok, err);
      return Promise.resolve(queues.get(key)?.shift() ?? []).then(ok, err);
    };
    return chain;
  };
  const tx = {
    select: () => ({ from: (t: unknown) => statement('select', t) }),
    insert: (t: unknown) => statement('insert', t),
    update: (t: unknown) => statement('update', t),
    delete: (t: unknown) => statement('delete', t),
  };
  const db = {
    transaction: async (fn: (t: typeof tx) => Promise<unknown>) => {
      transactions++;
      return fn(tx);
    },
  } as unknown as Db;
  return { db, calls, transactions: () => transactions, failOn: (k: string) => (failOn = k) };
}

const payload = (src: string) => ({ sourceSha: 'sha', releases: parseChangelog(src) });
const only = (calls: Call[], op: string, table: string) => calls.filter((c) => c.op === op && c.table === table);

describe('importChangelog (fake transaction)', () => {
  it('an empty payload deletes every release without a where clause, inside one transaction, and records the import', async () => {
    const f = fakeDb({ 'delete:releases': [[{ id: 1 }, { id: 2 }]] });
    const r = await importChangelog(f.db, payload(''));
    expect(f.transactions()).toBe(1);
    const del = only(f.calls, 'delete', 'releases');
    expect(del).toHaveLength(1);
    expect(del[0].where).toBe(false);
    expect(r).toMatchObject({ releases: 0, entries: 0, deleted: 2 });
    expect(only(f.calls, 'insert', 'imports')[0].values).toEqual({ sourceSha: 'sha', entriesCount: 0 });
  });

  it('a non-empty payload only deletes releases that left the file (where clause present)', async () => {
    const f = fakeDb({ 'delete:releases': [[{ id: 9 }]], 'insert:releases': [[{ id: 1 }]], 'insert:entries': [[{ id: 5 }]], 'insert:tags': [], 'select:tags': [[{ id: 3, name: 'files' }]] });
    const r = await importChangelog(f.db, payload('## [1.0.0] - 2026-01-01\n### Added\n- **A**: body. #files\n'));
    expect(only(f.calls, 'delete', 'releases')[0].where).toBe(true);
    expect(r).toMatchObject({ releases: 1, entries: 1, created: 1, deleted: 1 });
  });

  it('syncs tags: replaces the entry links, creates missing tags idempotently, links by tag id', async () => {
    const f = fakeDb({
      'select:releases': [[{ id: 1, releasedOn: '2026-01-01', summary: null }]],
      'select:entries': [[{ id: 5, slug: 'x', category: 'added', title: 'A', bodyMd: 'old', docsUrl: null, position: 0 }]],
      'select:entry_tags': [[{ name: 'old', id: 1 }]],
      'select:tags': [[{ id: 7, name: 'files' }]],
    });
    const src = '## [1.0.0] - 2026-01-01\n### Added\n- **A**: body. #files\n';
    const p = payload(src);
    p.releases[0].entries[0].slug = 'x';
    const r = await importChangelog(f.db, p);
    expect(r).toMatchObject({ created: 0, updated: 1 });
    expect(only(f.calls, 'update', 'entries')).toHaveLength(1);
    expect(only(f.calls, 'delete', 'entry_tags')).toHaveLength(1);
    const tagInsert = only(f.calls, 'insert', 'tags')[0];
    expect(tagInsert.values).toEqual({ name: 'files' });
    expect(tagInsert.conflict).toBe(true);
    expect(only(f.calls, 'insert', 'entry_tags')[0].values).toEqual({ entryId: 5, tagId: 7 });
  });

  it('leaves tags untouched when the set is unchanged and keeps untouched rows without updates', async () => {
    const f = fakeDb({
      'select:releases': [[{ id: 1, releasedOn: '2026-01-01', summary: null }]],
      'select:entries': [[{ id: 5, slug: 'x', category: 'added', title: 'A', bodyMd: 'body.', docsUrl: null, position: 0 }]],
      'select:entry_tags': [[{ name: 'files', id: 7 }]],
    });
    const p = payload('## [1.0.0] - 2026-01-01\n### Added\n- **A**: body. #files\n');
    p.releases[0].entries[0].slug = 'x';
    const r = await importChangelog(f.db, p);
    expect(r).toMatchObject({ created: 0, updated: 0, deleted: 0 });
    expect(only(f.calls, 'update', 'entries')).toHaveLength(0);
    expect(only(f.calls, 'delete', 'entry_tags')).toHaveLength(0);
    expect(only(f.calls, 'insert', 'tags')).toHaveLength(0);
  });

  it('deletes entries that left a release', async () => {
    const f = fakeDb({
      'select:releases': [[{ id: 1, releasedOn: '2026-01-01', summary: null }]],
      'select:entries': [[{ id: 8, slug: 'gone', category: 'added', title: null, bodyMd: 'z', docsUrl: null, position: 0 }]],
    });
    const r = await importChangelog(f.db, payload('## [1.0.0] - 2026-01-01\n'));
    expect(only(f.calls, 'delete', 'entries')).toHaveLength(1);
    expect(r.deleted).toBe(1);
  });

  it('propagates a failure from inside the transaction and never records the import', async () => {
    const f = fakeDb();
    f.failOn('delete:releases');
    await expect(importChangelog(f.db, payload(''))).rejects.toThrow(/boom/);
    expect(only(f.calls, 'insert', 'imports')).toHaveLength(0);
  });
});
