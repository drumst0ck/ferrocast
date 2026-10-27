/**
 * Build web/src/data/lastmod.json from git history (repo root). Used when Docker build
 * context is web/ only and has no .git.
 */
import { execSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = join(webRoot, '..');

function gitOk() {
  try {
    execSync('git rev-parse --is-inside-work-tree', { cwd: repoRoot, stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

if (!gitOk()) {
  console.warn('[sync-lastmod] Not a git checkout; leaving lastmod.json unchanged');
  process.exit(0);
}

const tracked = execSync('git ls-files web/', { cwd: repoRoot, encoding: 'utf8' })
  .split('\n')
  .map((l) => l.trim())
  .filter(Boolean)
  .map((p) => p.slice('web/'.length));

/** @type {Record<string, string>} */
const manifest = {};

for (const rel of tracked) {
  try {
    const iso = execSync(`git log -1 --format=%cI -- "web/${rel}"`, {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore'],
    }).trim();
    if (iso) manifest[rel] = iso;
  } catch {
    // untracked or missing in history
  }
}

const out = join(webRoot, 'src/data/lastmod.json');
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`[sync-lastmod] Wrote ${Object.keys(manifest).length} paths to src/data/lastmod.json`);
