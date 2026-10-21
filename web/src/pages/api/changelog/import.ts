import type { APIRoute } from 'astro';
import { getDb } from '../../../db/client';
import { handleImport } from '../../../lib/changelog/handler';
import { importChangelog } from '../../../lib/changelog/import';

export const prerender = false;

export const POST: APIRoute = ({ request }) =>
  handleImport(request, process.env.CHANGELOG_ADMIN_TOKEN ?? '', {
    importChangelog: async (payload) => {
      const db = getDb();
      if (!db) throw new Error('DATABASE_URL is not set');
      return importChangelog(db, payload);
    },
  });
