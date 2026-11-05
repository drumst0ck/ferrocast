import { readFileSync } from 'node:fs';
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

describe('/changelog/<version>/ page for an unknown version', () => {
  // Rendering the .astro page needs a live database, so this pins the source contract (structural check).
  const page = readFileSync(new URL('../../pages/changelog/[version].astro', import.meta.url), 'utf8');
  it('renders the site 404 page instead of an empty body with a Location header', () => {
    expect(page).not.toMatch(/Location\s*:/);
    expect(page).toMatch(/kind === 'missing'\) return Astro\.rewrite\('\/404'\)/);
  });
});
