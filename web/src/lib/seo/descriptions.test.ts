import { readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { factsCopy } from './facts';

/** One shared rule for every indexable non-docs page: meta description of 120-155 characters (docs enforce it in content.config.ts). */
const MIN = 120;
const MAX = 155;

const dir = new URL('./', import.meta.url);
const modules = readdirSync(dir).filter((f) => f.endsWith('.ts') && !f.endsWith('.test.ts') && f !== 'markup.ts');

function collect(o: unknown, path: string, out: [string, string][], depth = 0) {
  if (!o || typeof o !== 'object' || depth > 3) return;
  const d = (o as { description?: unknown }).description;
  if (typeof d === 'string') out.push([path, d]);
  for (const [k, v] of Object.entries(o)) collect(v, `${path}.${k}`, out, depth + 1);
}

describe('page meta descriptions', () => {
  it('finds the page descriptions it is supposed to guard', async () => {
    const found: [string, string][] = [];
    for (const f of modules) {
      const m = (await import(/* @vite-ignore */ new URL(f, dir).href)) as Record<string, unknown>;
      for (const [k, v] of Object.entries(m)) collect(v, `${f}:${k}`, found);
    }
    for (const lang of ['en', 'es'] as const) found.push([`facts.ts:factsCopy(${lang})`, factsCopy(lang).description]);
    expect(found.length).toBeGreaterThan(40);
    const bad = found.filter(([, d]) => d.length < MIN || d.length > MAX).map(([p, d]) => `${p} (${d.length})`);
    expect(bad).toEqual([]);
  });
});
