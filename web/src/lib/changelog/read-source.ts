import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

/** Paths checked in order (Docker build context is `web/` only). */
export function changelogSourceCandidates(cwd = process.cwd()): string[] {
  return [join(cwd, 'CHANGELOG.md'), join(cwd, '../CHANGELOG.md')];
}

export function resolveChangelogPath(cwd = process.cwd()): string | null {
  for (const path of changelogSourceCandidates(cwd)) {
    if (existsSync(path)) return path;
  }
  return null;
}

export function readChangelogSource(cwd = process.cwd()): string | null {
  const path = resolveChangelogPath(cwd);
  if (!path) return null;
  return readFileSync(path, 'utf8');
}
