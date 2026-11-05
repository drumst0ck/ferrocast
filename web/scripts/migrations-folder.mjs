import { fileURLToPath } from 'node:url';

/** Filesystem path of the drizzle folder next to `scripts/`; decodes %20 and non-ASCII, unlike URL.pathname. */
export function migrationsFolder(fromUrl) {
  return fileURLToPath(new URL('../drizzle', fromUrl));
}
