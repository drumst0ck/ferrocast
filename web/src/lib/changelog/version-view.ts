import { loadRelease } from './load';
import { releaseMeta } from './meta';
import type { ReleaseView } from './query';

export type VersionView =
  | { kind: 'unavailable' }
  | { kind: 'missing' }
  | { kind: 'ok'; release: ReleaseView; meta: ReturnType<typeof releaseMeta> };

/** Resolves what /changelog/<version>/ should show; never throws when the database is down. */
export async function loadVersionView(version: string): Promise<VersionView> {
  const { release, ok } = await loadRelease(version.toLowerCase());
  if (!ok) return { kind: 'unavailable' };
  if (!release) return { kind: 'missing' };
  return { kind: 'ok', release, meta: releaseMeta(release) };
}
