// Applies SQL migrations from ./drizzle. Run as a Coolify pre-deploy command: node scripts/migrate.mjs
import { drizzle } from 'drizzle-orm/postgres-js';
import { migrate } from 'drizzle-orm/postgres-js/migrator';
import postgres from 'postgres';

const url = process.env.DATABASE_URL;
if (!url) {
  console.error('DATABASE_URL is not set');
  process.exit(1);
}
const sql = postgres(url, { max: 1, onnotice: () => {} });
try {
  await migrate(drizzle(sql), { migrationsFolder: new URL('../drizzle', import.meta.url).pathname });
  console.log('migrations applied');
} finally {
  await sql.end();
}
