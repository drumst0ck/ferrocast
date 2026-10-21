import { getDb } from '../../db/client';
import { getRelease, listReleases, type ReleaseView } from './query';

/** Never throws: pages render a fallback block when the database is unreachable. */
export async function loadReleases(): Promise<ReleaseView[] | null> {
  const db = getDb();
  if (!db) return null;
  try {
    return await listReleases(db);
  } catch (e) {
    console.error('changelog query failed', e);
    return null;
  }
}

export async function loadRelease(version: string): Promise<{ release: ReleaseView | null; ok: boolean }> {
  const db = getDb();
  if (!db) return { release: null, ok: false };
  try {
    return { release: await getRelease(db, version), ok: true };
  } catch (e) {
    console.error('changelog query failed', e);
    return { release: null, ok: false };
  }
}

export const CATEGORY_LABEL: Record<string, string> = {
  added: 'Added',
  changed: 'Changed',
  fixed: 'Fixed',
  removed: 'Removed',
  security: 'Security',
  limitation: 'Known limitations',
};
export const CATEGORY_ORDER = ['added', 'changed', 'fixed', 'removed', 'security', 'limitation'];

export const versionLabel = (v: string) => (v === 'unreleased' ? 'Unreleased' : v);
