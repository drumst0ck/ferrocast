// Usage: npm run changelog:push -- [--file ../CHANGELOG.md] [--url https://ferrocast.app] [--dry-run]
// Env:   CHANGELOG_ADMIN_TOKEN (required unless --dry-run)
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { parseChangelog } from '../../src/lib/changelog/parse';
import { resolveChangelogPath } from '../../src/lib/changelog/read-source';

const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const file = opt('file', resolveChangelogPath() ?? 'CHANGELOG.md');
const url = opt('url', process.env.CHANGELOG_URL ?? 'https://ferrocast.app').replace(/\/$/, '');
const source = readFileSync(file, 'utf8');

let releases;
try {
  releases = parseChangelog(source);
} catch (e) {
  console.error(`${file}: ${(e as Error).message}`);
  process.exit(1);
}
const payload = { sourceSha: createHash('sha256').update(source).digest('hex'), releases };
const total = releases.reduce((n, r) => n + r.entries.length, 0);

if (args.includes('--dry-run')) {
  console.log(JSON.stringify(payload, null, 2));
  console.error(`dry run: ${releases.length} releases, ${total} entries`);
  process.exit(0);
}

const token = process.env.CHANGELOG_ADMIN_TOKEN;
if (!token) {
  console.error('CHANGELOG_ADMIN_TOKEN is not set');
  process.exit(1);
}
const res = await fetch(`${url}/api/changelog/import/`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
  body: JSON.stringify(payload),
});
const body = await res.text();
console.log(`${res.status} ${body}`);
process.exit(res.ok ? 0 : 1);
