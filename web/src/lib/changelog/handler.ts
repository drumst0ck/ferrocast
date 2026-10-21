import { timingSafeEqual } from 'node:crypto';
import type { ImportResult } from './import';
import { validatePayload, type ImportPayload } from './validate';

const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

function tokenMatches(header: string | null, token: string): boolean {
  const given = Buffer.from((header ?? '').replace(/^Bearer /, ''));
  const want = Buffer.from(token);
  return given.length === want.length && timingSafeEqual(given, want);
}

/** Pure request handler so it can be tested without Astro. `token` comes from CHANGELOG_ADMIN_TOKEN. */
export async function handleImport(request: Request, token: string, deps: { importChangelog: (p: ImportPayload) => Promise<ImportResult> }): Promise<Response> {
  if (!token) return json(503, { error: 'import is disabled: CHANGELOG_ADMIN_TOKEN is not configured' });
  if (!tokenMatches(request.headers.get('authorization'), token)) return json(401, { error: 'unauthorized' });
  let payload: ImportPayload;
  try {
    payload = validatePayload(await request.json());
  } catch (e) {
    return json(422, { error: e instanceof Error ? e.message : 'invalid payload' });
  }
  try {
    return json(200, { ok: true, ...(await deps.importChangelog(payload)) });
  } catch (e) {
    console.error('changelog import failed', e);
    return json(500, { error: 'import failed' });
  }
}
