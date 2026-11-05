import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { allTools } from '../seo/tools';

/** Builds a throwaway git checkout, points process.cwd() at it and loads lastmod.ts fresh (webRoot is read at import). */
const cwd = process.cwd();
let dir: string;

const write = (rel: string, body = 'x') => {
  mkdirSync(dirname(join(dir, rel)), { recursive: true });
  writeFileSync(join(dir, rel), body);
};
const commit = (files: string[], iso: string) => {
  const env = { ...process.env, GIT_AUTHOR_DATE: iso, GIT_COMMITTER_DATE: iso, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null' };
  const git = (...a: string[]) => execFileSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@t', '-c', 'commit.gpgsign=false', ...a], { cwd: dir, env, stdio: 'ignore' });
  git('add', ...files);
  git('commit', '-m', 'c');
};
async function load() {
  vi.resetModules();
  process.chdir(dir);
  return (await import('./lastmod')).lastmodForPageUrl;
}

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), 'lastmod-'));
  write('src/content/docs/es/docs/index.md'); // es-translations scans this folder at import
});
afterEach(() => process.chdir(cwd));

describe('lastmodForPageUrl with git', () => {
  it('prefers the git commit date over a stale lastmod.json entry', async () => {
    execFileSync('git', ['init', '-q'], { cwd: dir });
    write('src/pages/changelog/index.astro');
    write('CHANGELOG.md');
    write('src/data/lastmod.json', JSON.stringify({ 'CHANGELOG.md': '2026-10-29T00:00:00+00:00', 'src/pages/changelog/index.astro': '2026-10-29T00:00:00+00:00' }));
    commit(['.'], '2026-11-03T12:00:00+00:00');
    const lastmod = await load();
    expect(lastmod('https://ferrocast.app/changelog/').toISOString()).toBe('2026-11-03T12:00:00.000Z');
  });

  it('falls back to lastmod.json when there is no git checkout', async () => {
    write('src/pages/changelog/index.astro');
    write('CHANGELOG.md');
    write('src/data/lastmod.json', JSON.stringify({ 'CHANGELOG.md': '2026-10-29T00:00:00+00:00', 'src/pages/changelog/index.astro': '2026-10-20T00:00:00+00:00' }));
    const lastmod = await load();
    expect(lastmod('https://ferrocast.app/changelog/').toISOString()).toBe('2026-10-29T00:00:00.000Z');
  });

  it('/tools/<slug>/ follows the docs MDX its body comes from', async () => {
    const tool = allTools().find((t) => t.slug !== 'organize-pdf')!;
    execFileSync('git', ['init', '-q'], { cwd: dir });
    write('src/pages/tools/[slug].astro');
    write('src/data/tools.json');
    write('src/lib/seo/tools.ts');
    write('src/lib/seo/tool-guides.ts');
    commit(['.'], '2026-10-29T12:00:00+00:00');
    write(`src/content/docs/docs/files/tools/${tool.docSlug}.mdx`, 'new body');
    commit(['.'], '2026-11-03T12:00:00+00:00');
    const lastmod = await load();
    expect(lastmod(`https://ferrocast.app/tools/${tool.slug}/`).toISOString()).toBe('2026-11-03T12:00:00.000Z');
  });

  it('/tools/<slug>/ also follows tools.ts and tool-guides.ts', async () => {
    const tool = allTools().find((t) => t.slug !== 'organize-pdf')!;
    execFileSync('git', ['init', '-q'], { cwd: dir });
    write('src/pages/tools/[slug].astro');
    write('src/data/tools.json');
    write('src/lib/seo/tools.ts');
    write('src/lib/seo/tool-guides.ts');
    commit(['.'], '2026-10-29T12:00:00+00:00');
    write('src/lib/seo/tool-guides.ts', 'changed');
    commit(['.'], '2026-11-05T12:00:00+00:00');
    const lastmod = await load();
    expect(lastmod(`https://ferrocast.app/tools/${tool.slug}/`).toISOString()).toBe('2026-11-05T12:00:00.000Z');
  });
});
