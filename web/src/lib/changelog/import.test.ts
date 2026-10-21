import { afterAll, beforeEach, describe, expect, it } from 'vitest';
import { sql } from 'drizzle-orm';
import { createDb } from '../../db/client';
import { migrateTestDb } from './test-helpers';
import { importChangelog } from './import';
import { getRelease, listReleases } from './query';
import { handleImport } from './handler';
import { parseChangelog } from './parse';

const url = process.env.TEST_DATABASE_URL;
const md = (extra = '') => `## [Unreleased]\n### Added\n- **A**: first. #files\n- **B**: second.${extra}\n### Fixed\n- Fix one.\n## [0.1.0] - 2026-10-15\n### Added\n- **S**: search.\n`;
const payload = (src: string) => ({ sourceSha: 'sha-' + src.length, releases: parseChangelog(src) });

describe.skipIf(!url)('importChangelog (postgres)', () => {
  const { db, close } = createDb(url ?? 'postgres://x');
  beforeEach(async () => {
    await migrateTestDb(url!);
    await db.execute(sql`truncate releases, tags, imports restart identity cascade`);
  });
  afterAll(() => close());

  it('imports releases, entries and tags and renders safe html', async () => {
    const r = await importChangelog(db, payload(md('\n- <script>alert(1)</script> **x**')));
    expect(r).toMatchObject({ releases: 2, entries: 5 });
    const list = await listReleases(db);
    expect(list.map((x) => x.version)).toEqual(['unreleased', '0.1.0']);
    expect(list[0].entries.map((e) => e.title)).toEqual(['A', 'B', null, null]);
    expect(list[0].entries[0].tags).toEqual(['files']);
    expect(list[0].entries[2].bodyHtml).not.toContain('<script');
    expect(list[0].entries[2].bodyHtml).toContain('<strong>x</strong>');
  });

  it('is idempotent: a second identical push changes no row and keeps updated_at', async () => {
    await importChangelog(db, payload(md()));
    const before = await getRelease(db, 'unreleased');
    const r2 = await importChangelog(db, payload(md()));
    const after = await getRelease(db, 'unreleased');
    expect(r2).toMatchObject({ created: 0, updated: 0, deleted: 0 });
    expect(after!.entries.map((e) => +e.updatedAt)).toEqual(before!.entries.map((e) => +e.updatedAt));
  });

  it('updates changed entries and deletes entries and releases that left the file', async () => {
    await importChangelog(db, payload(md()));
    const src2 = `## [Unreleased]\n### Added\n- **A**: first, reworded. #files\n`;
    const r = await importChangelog(db, payload(src2));
    expect(r.deleted).toBeGreaterThanOrEqual(3);
    expect(await getRelease(db, '0.1.0')).toBeNull();
    const rel = await getRelease(db, 'unreleased');
    expect(rel!.entries).toHaveLength(1);
    expect(rel!.entries[0].bodyMd).toBe('first, reworded.');
  });
});

describe('handleImport', () => {
  const fake = { importChangelog: async () => ({ releases: 1, entries: 1, created: 1, updated: 0, deleted: 0 }) };
  const req = (body: unknown, auth?: string) =>
    new Request('http://x/api/changelog/import', { method: 'POST', headers: auth ? { authorization: auth } : {}, body: JSON.stringify(body) });
  const good = { sourceSha: 'abc', releases: [{ version: '1.0.0', releasedOn: null, summary: null, entries: [] }] };

  it('401 without a valid bearer token', async () => {
    expect((await handleImport(req(good), 'secret', fake)).status).toBe(401);
    expect((await handleImport(req(good, 'Bearer nope'), 'secret', fake)).status).toBe(401);
  });
  it('503 when no token is configured, never accepting an empty one', async () => {
    expect((await handleImport(req(good, 'Bearer '), '', fake)).status).toBe(503);
  });
  it('422 on a malformed payload', async () => {
    const res = await handleImport(req({ sourceSha: 1 }, 'Bearer secret'), 'secret', fake);
    expect(res.status).toBe(422);
    expect((await res.json()).error).toMatch(/sourceSha/);
  });
  it('200 with counts on success', async () => {
    const res = await handleImport(req(good, 'Bearer secret'), 'secret', fake);
    expect(res.status).toBe(200);
    expect(await res.json()).toMatchObject({ ok: true, entries: 1 });
  });
});
