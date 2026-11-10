import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const webDir = join(dirname(fileURLToPath(import.meta.url)), '..');
const dockerfile = readFileSync(join(webDir, 'Dockerfile'), 'utf8');

/** Paths (relative to web/) copied from the build context into the run stage. */
const copied = new Set(
  [...dockerfile.matchAll(/^COPY\s+(?!--from)(\S+)\s+\S+\s*$/gm)].map((m) => normalize(m[1])),
);

function relativeImports(file) {
  const src = readFileSync(join(webDir, file), 'utf8');
  const specs = [...src.matchAll(/(?:from\s+|import\s*\(?\s*)['"](\.{1,2}\/[^'"]+)['"]/g)].map((m) => m[1]);
  return specs.map((s) => normalize(join(dirname(file), s)));
}

function closure(entry) {
  const seen = new Set();
  const todo = [normalize(entry)];
  while (todo.length) {
    const f = todo.pop();
    if (seen.has(f)) continue;
    seen.add(f);
    // dist/ is produced by the build stage and copied wholesale
    for (const dep of relativeImports(f)) if (!dep.startsWith('dist/') && existsSync(join(webDir, dep))) todo.push(dep);
  }
  return seen;
}

describe('Dockerfile run stage', () => {
  it.each(['scripts/production-server.mjs', 'scripts/migrate.mjs'])('copies every module %s imports, transitively', (entry) => {
    const missing = [...closure(entry)].filter((f) => !copied.has(f));
    expect(missing).toEqual([]);
  });
});
