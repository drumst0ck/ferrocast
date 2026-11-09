import { drizzle, type PostgresJsDatabase } from 'drizzle-orm/postgres-js';
import postgres from 'postgres';
import * as schema from './schema';

export type Db = PostgresJsDatabase<typeof schema>;

/** Shared pool options. `statement_timeout` keeps a stuck query from holding a connection (and the page) forever. */
export const poolOptions = {
  max: 5,
  idle_timeout: 20,
  connect_timeout: 5,
  connection: { statement_timeout: 10000 },
};

let cached: Db | null = null;

/** Lazily connects with DATABASE_URL (read at runtime, never at build). Returns null when unset. */
export function getDb(): Db | null {
  const url = process.env.DATABASE_URL;
  if (!url) return null;
  cached ??= drizzle(postgres(url, poolOptions), { schema });
  return cached;
}

export function createDb(url: string): { db: Db; close: () => Promise<void> } {
  const sql = postgres(url, { max: 2, onnotice: () => {} });
  return { db: drizzle(sql, { schema }), close: () => sql.end() };
}
