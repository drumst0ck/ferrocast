import { describe, expect, it } from 'vitest';
import { migrationsFolder } from './migrations-folder.mjs';

describe('migrationsFolder', () => {
  it('decodes spaces and non-ASCII characters in the checkout path', () => {
    expect(migrationsFolder('file:///Users/me/My%20Repo/ca%C3%B1a/web/scripts/migrate.mjs')).toBe('/Users/me/My Repo/caña/web/drizzle');
  });
  it('resolves the real drizzle folder next to scripts/', () => {
    expect(migrationsFolder(import.meta.url)).toMatch(/web\/drizzle$/);
  });
});
