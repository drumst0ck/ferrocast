import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { readChangelogSource, resolveChangelogPath } from './read-source';

function checkout(withRoot: boolean) {
  const root = mkdtempSync(join(tmpdir(), 'cl-'));
  const web = join(root, 'web');
  mkdirSync(web);
  writeFileSync(join(web, 'CHANGELOG.md'), 'web copy');
  if (withRoot) writeFileSync(join(root, 'CHANGELOG.md'), 'root file');
  return { root, web };
}

describe('changelog source resolution', () => {
  it('prefers the root file over the possibly stale web copy', () => {
    const { root, web } = checkout(true);
    expect(resolveChangelogPath(web)).toBe(join(root, 'CHANGELOG.md'));
    expect(readChangelogSource(web)).toBe('root file');
  });
  it('falls back to the web copy when built from web/ only (Docker)', () => {
    const { web } = checkout(false);
    expect(resolveChangelogPath(web)).toBe(join(web, 'CHANGELOG.md'));
    expect(readChangelogSource(web)).toBe('web copy');
  });
  it('returns null when neither exists', () => {
    const root = mkdtempSync(join(tmpdir(), 'cl-'));
    const web = join(root, 'web');
    mkdirSync(web);
    expect(resolveChangelogPath(web)).toBe(null);
  });
});
