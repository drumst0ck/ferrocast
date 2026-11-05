import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { GET as rssGET } from '../../pages/changelog/rss.xml';
import { GET as jsonGET } from '../../pages/changelog.json';

const ctx = { site: new URL('https://ferrocast.app') } as never;
let saved: string | undefined;
beforeEach(() => {
  saved = process.env.DATABASE_URL;
  delete process.env.DATABASE_URL;
});
afterEach(() => {
  if (saved === undefined) delete process.env.DATABASE_URL;
  else process.env.DATABASE_URL = saved;
});

describe('feeds when the database is unavailable', () => {
  it('JSON feed answers 503 no-store, not an empty cached feed', async () => {
    const res = await jsonGET(ctx);
    expect(res.status).toBe(503);
    expect(res.headers.get('Cache-Control')).toBe('no-store');
    expect(res.headers.get('Retry-After')).toBeTruthy();
  });
  it('RSS feed answers 503 no-store, not an empty cached feed', async () => {
    const res = await rssGET(ctx);
    expect(res.status).toBe(503);
    expect(res.headers.get('Cache-Control')).toBe('no-store');
    expect(res.headers.get('Retry-After')).toBeTruthy();
  });
});
