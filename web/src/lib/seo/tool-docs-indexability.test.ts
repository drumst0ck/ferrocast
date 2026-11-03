import { existsSync } from 'node:fs';
import { expect, it } from 'vitest';
import manifest from '../../data/tool-docs-indexability.json';
import { isToolDocExcluded, toolDocEntry } from '../../../tool-docs-indexability.mjs';
import { allTools } from './tools';
import { applyToolDocIndexability } from '../docs/tool-doc-head';

it('has one entry per tool docs page and each target is its /tools/ page', () => {
  const tools = allTools();
  expect(manifest.entries.map((e) => e.path).sort()).toEqual(tools.map((t) => `/docs/files/tools/${t.docSlug}/`).sort());
  for (const e of manifest.entries) {
    expect(existsSync(`src/content/docs${e.path.replace(/\/$/, '')}.mdx`), e.path).toBe(true);
    if (e.decision === 'canonical') expect(e.target).toBe(`/tools/${e.path.split('/').filter(Boolean).pop()}/`);
    else expect(e.target).toBeNull();
    expect(e.reason.length, e.path).toBeGreaterThan(20);
  }
});
it('excludes only the English docs paths from the sitemap and resolves ES fallbacks to the same entry', () => {
  expect(isToolDocExcluded('https://ferrocast.app/docs/files/tools/compress/')).toBe(true);
  expect(isToolDocExcluded('https://ferrocast.app/es/docs/files/tools/compress/')).toBe(false);
  expect(toolDocEntry('/es/docs/files/tools/compress/')?.path).toBe('/docs/files/tools/compress/');
  for (const p of ['/tools/compress/', '/tools/', '/docs/files/conversions/', '/docs/files/typed-commands/', '/convert/jpg-to-png/', '/files/', '/'])
    expect(isToolDocExcluded(p), p).toBe(false);
});
it('rewrites the head: canonical to the tool page, or noindex; never hreflang', () => {
  const head = [
    { tag: 'link', attrs: { rel: 'canonical', href: 'https://ferrocast.app/docs/files/tools/compress/' } },
    { tag: 'meta', attrs: { property: 'og:url', content: 'https://ferrocast.app/docs/files/tools/compress/' } },
    { tag: 'link', attrs: { rel: 'alternate', hreflang: 'es', href: 'https://ferrocast.app/es/docs/files/tools/compress/' } },
  ];
  const c = applyToolDocIndexability(head, { path: '/docs/files/tools/compress/', decision: 'canonical', target: '/tools/compress/' });
  expect(c).toHaveLength(2);
  expect(c[0].attrs?.href).toBe('https://ferrocast.app/tools/compress/');
  expect(c[1].attrs?.content).toBe('https://ferrocast.app/tools/compress/');
  const n = applyToolDocIndexability(head, { path: '/docs/files/tools/compress/', decision: 'noindex', target: '/tools/compress/' });
  expect(n[0].attrs?.href).toBe('https://ferrocast.app/docs/files/tools/compress/');
  expect(n.at(-1)?.attrs).toEqual({ name: 'robots', content: 'noindex, follow' });
  expect(head).toHaveLength(3);
});
