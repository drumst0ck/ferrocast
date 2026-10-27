// Keep web/CHANGELOG.md in sync with the repo root when building from a full checkout.
import { copyFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const rootChangelog = join(webRoot, '../CHANGELOG.md');
const webChangelog = join(webRoot, 'CHANGELOG.md');

if (existsSync(rootChangelog)) {
  copyFileSync(rootChangelog, webChangelog);
} else if (!existsSync(webChangelog)) {
  console.warn('[sync-changelog] No CHANGELOG.md in web/ or parent; sitemap may omit version URLs');
}
