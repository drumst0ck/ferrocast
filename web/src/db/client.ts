import { drizzle, type PostgresJsDatabase } from 'drizzle-orm/postgres-js';
import postgres from 'postgres';
import * as schema from './schema';

export type Db = PostgresJsDatabase<typeof schema>;

let cached: Db | null = null;

/** Lazily connects with DATABASE_URL (read at runtime, never at build). Returns null when unset. */
export function getDb(): Db | null {
  const url = process.env.DATABASE_URL;
  if (!url) return null;
  cached ??= drizzle(postgres(url, { max: 5, idle_timeout: 20, connect_timeout: 5 }), { schema });
  return cached;
}

export function createDb(url: string): { db: Db; close: () => Promise<void> } {
  const sql = postgres(url, { max: 2, onnotice: () => {} });
  return { db: drizzle(sql, { schema }), close: () => sql.end() };
}
