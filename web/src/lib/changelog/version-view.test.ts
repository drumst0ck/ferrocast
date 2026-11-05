import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { loadVersionView } from './version-view';

let saved: string | undefined;
beforeEach(() => {
  saved = process.env.DATABASE_URL;
  delete process.env.DATABASE_URL;
});
afterEach(() => {
  if (saved === undefined) delete process.env.DATABASE_URL;
  else process.env.DATABASE_URL = saved;
});

describe('loadVersionView', () => {
  it('reports unavailable (503) without throwing when the database is down', async () => {
    const view = await loadVersionView('0.5.0');
    expect(view).toEqual({ kind: 'unavailable' });
  });
});
