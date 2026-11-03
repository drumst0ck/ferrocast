// Decides, per /docs/files/tools/<slug>/ page, canonical (substantial overlap with /tools/<slug>/) or noindex,follow
// (own distinct content), from a build in dist/ made with the manifest disabled or enabled (content is the same).
// Writes decisions.csv and src/data/tool-docs-indexability.json. Run: npm run build && node seo/tool-docs/review.mjs
import fs from 'node:fs';
const root = new URL('./', import.meta.url);
const dist = new URL('../../dist/client/', import.meta.url);
const docsDir = new URL('../../src/content/docs/docs/files/tools/', import.meta.url);
const OVERLAP_MIN = 0.6; // share of the doc's content words that also appear on the tool page
const stop = new Set("the and for with that this from are you your into can not has have was but its all any one use when then they them will also each than only more which what who how out our per via".split(' '));
const words = (html) => html.replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/g, ' ').replace(/<[^>]+>/g, ' ').replace(/&[a-z#0-9]+;/g, ' ').toLowerCase().replace(/[^a-z0-9À-￿ ]+/g, ' ').split(/\s+/).filter((w) => w.length > 2 && !stop.has(w));
const flat = (html) => html.replace(/<[^>]+>/g, '').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/\s+/g, '');
const hist = JSON.parse(fs.readFileSync(new URL('sources/search-signals.json', root), 'utf8'));
const rows = [];
for (const f of fs.readdirSync(docsDir).filter((n) => n.endsWith('.mdx')).sort()) {
  const slug = f.slice(0, -4);
  const body = fs.readFileSync(new URL(f, docsDir), 'utf8').split('## Related')[0];
  const docHtml = fs.readFileSync(new URL(`docs/files/tools/${slug}/index.html`, dist), 'utf8');
  const toolHtml = fs.readFileSync(new URL(`tools/${slug}/index.html`, dist), 'utf8');
  const toolMain = toolHtml.match(/<main[\s\S]*<\/main>/)[0];
  const docMain = docHtml.match(/<div class="sl-markdown-content">([\s\S]*?)<\/div>\s*<\/div>\s*<\/main>/)?.[1] ?? '';
  const dw = words(docMain), tw = new Set(words(toolMain));
  const overlap = dw.filter((w) => tw.has(w)).length / dw.length;
  const facts = [...body.matchAll(/`([^`]+)`/g)].map((m) => m[1])
    .concat([...body.matchAll(/<kbd>([^<]+)<\/kbd>/g)].map((m) => m[1])) // single keys: a chord may be written as ⌘←/⌘→ on the page
    .concat([...body.matchAll(/\*\*([^*]+)\*\*/g)].map((m) => m[1]));
  const flatTool = flat(toolMain);
  const missing = [...new Set(facts.filter((x) => !flatTool.includes(x.replace(/\s+/g, '')))) ];
  const canonical = overlap >= OVERLAP_MIN && missing.length === 0;
  const h = hist[slug];
  let reason = canonical
    ? `Substantial overlap: ${(overlap * 100).toFixed(0)}% of the doc's content words and all ${facts.length} keys, commands and button names are on /tools/${slug}/`
    : `Own content: ${(overlap * 100).toFixed(0)}% overlap; not on the tool page: ${missing.join(' | ')}`;
  if (h) reason += `. ${h}`;
  rows.push({ url: `https://ferrocast.app/docs/files/tools/${slug}/`, decision: canonical ? 'canonical' : 'noindex', target: canonical ? `https://ferrocast.app/tools/${slug}/` : '', overlap: overlap.toFixed(2), facts: facts.length, missing: missing.join(' | '), reason });
}
const q = (v) => '"' + String(v).replaceAll('"', '""') + '"';
fs.writeFileSync(new URL('decisions.csv', root), Object.keys(rows[0]).join(',') + '\n' + rows.map((r) => Object.values(r).map(q).join(',')).join('\n') + '\n');
fs.writeFileSync(new URL('../../src/data/tool-docs-indexability.json', root), JSON.stringify({ enabled: true, entries: rows.map((r) => ({ path: new URL(r.url).pathname, decision: r.decision, target: r.decision === 'canonical' ? new URL(r.target).pathname : null, reason: r.reason })) }, null, 2) + '\n');
console.log({ total: rows.length, canonical: rows.filter((r) => r.decision === 'canonical').length, noindex: rows.filter((r) => r.decision === 'noindex').length });
