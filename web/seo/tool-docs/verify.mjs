import fs from 'node:fs';
import assert from 'node:assert/strict';
import manifest from '../../src/data/tool-docs-indexability.json' with { type: 'json' };
const dist = new URL('../../dist/client/', import.meta.url);
const read = (p) => fs.readFileSync(new URL('.' + p + 'index.html', dist), 'utf8');
const sitemap = fs.readdirSync(dist).filter((n) => /^sitemap-\d+\.xml$/.test(n)).map((n) => fs.readFileSync(new URL(n, dist), 'utf8')).join('');
const locs = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1]);
const inMap = (p) => locs.includes('https://ferrocast.app' + p);
const link = (html, rel) => html.match(new RegExp(`<link[^>]*rel="${rel}"[^>]*>`))?.[0] ?? '';
const href = (tag) => tag.match(/href="([^"]+)"/)?.[1];
const excluded = new Set(manifest.entries.map((e) => e.path));
for (const e of manifest.entries) {
  const slug = e.path.split('/').filter(Boolean).pop();
  const doc = read(e.path), tool = read(`/tools/${slug}/`);
  assert.equal(href(link(doc, 'canonical')), e.decision === 'canonical' ? 'https://ferrocast.app' + e.target : 'https://ferrocast.app' + e.path, e.path);
  assert.equal(/<meta name="robots" content="noindex/.test(doc), e.decision === 'noindex', e.path);
  assert.ok(!/rel="alternate"[^>]*hreflang|hreflang[^>]*rel="alternate"/.test(doc), `${e.path} must carry no hreflang`);
  assert.ok(!inMap(e.path), `${e.path} must not be in the sitemap`);
  assert.ok(doc.includes(`href="/tools/${slug}/"`), `${e.path} must link to its tool page`);
  assert.ok(inMap(`/tools/${slug}/`), `/tools/${slug}/ must be in the sitemap`);
  assert.equal(href(link(tool, 'canonical')), `https://ferrocast.app/tools/${slug}/`);
  assert.ok(!/<meta name="robots" content="noindex/.test(tool), `/tools/${slug}/ must be indexable`);
  assert.ok(tool.includes(`href="${e.path}"`), `/tools/${slug}/ must link to its doc`);
  const es = fs.existsSync(new URL('.' + '/es' + e.path + 'index.html', dist)) ? read('/es' + e.path) : '';
  if (es) {
    assert.equal(href(link(es, 'canonical')), e.decision === 'canonical' ? 'https://ferrocast.app' + e.target : 'https://ferrocast.app' + e.path, '/es' + e.path);
    assert.ok(/<meta name="robots" content="noindex/.test(es), '/es' + e.path + ' stays noindex');
    assert.ok(!/hreflang/.test(es), '/es' + e.path + ' must carry no hreflang');
  }
}
function walk(dir) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = new URL(ent.name + (ent.isDirectory() ? '/' : ''), dir);
    if (ent.isDirectory()) walk(p);
    else if (ent.name.endsWith('.html')) for (const tag of fs.readFileSync(p, 'utf8').matchAll(/<link[^>]*rel="alternate"[^>]*hreflang[^>]*>/g)) {
      const u = href(tag[0]);
      if (u) assert.ok(!excluded.has(new URL(u).pathname), `${p.pathname}: hreflang -> ${u}`);
    }
  }
}
walk(dist);
const count = (re) => locs.filter((u) => re.test(u)).length;
console.log({ sitemapTotal: locs.length, docsTools: count(/\/docs\/files\/tools\//), tools: count(/ferrocast\.app\/tools\//), convert: count(/\/convert\//) });
console.log(`Verified ${manifest.entries.length} tool docs: canonical/robots/hreflang/sitemap/links in both directions, ES fallbacks, all hreflang targets.`);
