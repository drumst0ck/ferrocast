/**
 * Build-time critical CSS for Starlight docs and /convert/ pages.
 * Class names in the docs bundle are hashed, so this runs against dist/client
 * during astro:build:done (also: npm run critical:pages).
 */
import Beasties from 'beasties';
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const ASYNC_CSS = (href) =>
  `<link rel="preload" href="${href}" as="style"><link rel="stylesheet" href="${href}" media="print" onload="this.media='all'"><noscript><link rel="stylesheet" href="${href}"></noscript>`;

const VOID_TAGS = new Set([
  'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr',
]);

function walkHtml(dir, acc = []) {
  let names;
  try {
    names = readdirSync(dir);
  } catch {
    return acc;
  }
  for (const name of names) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walkHtml(p, acc);
    else if (name.endsWith('.html')) acc.push(p);
  }
  return acc;
}

function stripScripts(html) {
  return html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '');
}

/** Close tags left open after truncating a document (skips raw script/style bodies). */
export function closeOpenTags(html) {
  const raw = new Set(['script', 'style']);
  const stack = [];
  let i = 0;
  while (i < html.length) {
    const top = stack[stack.length - 1];
    if (top && raw.has(top)) {
      const end = html.toLowerCase().indexOf(`</${top}>`, i);
      if (end < 0) break;
      stack.pop();
      i = end + top.length + 3;
      continue;
    }
    const lt = html.indexOf('<', i);
    if (lt < 0) break;
    if (html.startsWith('<!--', lt)) {
      const end = html.indexOf('-->', lt);
      i = end < 0 ? html.length : end + 3;
      continue;
    }
    const gt = html.indexOf('>', lt);
    if (gt < 0) break;
    const rawTag = html.slice(lt + 1, gt).trim();
    i = gt + 1;
    if (!rawTag || rawTag.startsWith('!') || rawTag.startsWith('?')) continue;
    const closing = rawTag.startsWith('/');
    const name = (closing ? rawTag.slice(1) : rawTag).split(/\s/)[0].toLowerCase();
    if (!name) continue;
    if (closing) {
      const at = stack.lastIndexOf(name);
      if (at >= 0) stack.splice(at, 1);
      continue;
    }
    const selfClose = rawTag.endsWith('/') || VOID_TAGS.has(name);
    if (!selfClose) stack.push(name);
  }
  return html + stack.reverse().map((t) => `</${t}>`).join('');
}

function blockingStylesheets(html) {
  /** @type {{ tag: string, href: string }[]} */
  const links = [];
  for (const match of html.matchAll(/<link\b[^>]*>/g)) {
    const tag = match[0];
    if (!tag.includes('rel="stylesheet"')) continue;
    if (/\smedia=/.test(tag)) continue;
    const href = tag.match(/href="([^"]+\.css)"/);
    if (!href) continue;
    links.push({ tag, href: href[1] });
  }
  return links;
}

function docsShell(html, ecHref) {
  const stripped = stripScripts(html);
  const idx = stripped.indexOf('sl-markdown-content');
  if (idx < 0) return null;
  const tableEnd = stripped.indexOf('</table>', idx);
  const pEnd = stripped.indexOf('</p>', idx);
  const cutAt = tableEnd >= 0 ? tableEnd + '</table>'.length : pEnd >= 0 ? pEnd + 4 : -1;
  if (cutAt < 0) return null;
  let cut = stripped.slice(0, cutAt);
  cut +=
    '<pre><code>sample</code></pre>' +
    '<aside class="starlight-aside starlight-aside--caution"><p>Note</p></aside>' +
    '<div class="expressive-code"><figure class="frame not-content"><figcaption class="header"></figcaption><pre data-language="bash"><code><div class="ec-line"><div class="code"><span>code</span></div></div></code></pre></figure></div>';
  if (ecHref && !cut.includes(ecHref)) {
    cut = cut.replace('</head>', `<link rel="stylesheet" href="${ecHref}"></head>`);
  }
  return closeOpenTags(cut);
}

function convertShell(html) {
  const stripped = stripScripts(html);
  const lead = stripped.indexOf('class="lead"');
  const pEnd = lead >= 0 ? stripped.indexOf('</p>', lead) : stripped.indexOf('</h1>');
  if (pEnd < 0) return null;
  const cut =
    stripped.slice(0, pEnd + 4) +
    '<h2>How</h2><ol><li>Step</li></ol><pre><code>brew install</code></pre><ul class="chips"><li><a href="/convert/">pair</a></li></ul>';
  return closeOpenTags(cut);
}

async function extractCritical(clientDir, shell) {
  const beasties = new Beasties({
    path: clientDir,
    publicPath: '/',
    preload: 'media',
    compress: true,
    logLevel: 'warn',
    pruneSource: false,
    mergeStylesheets: true,
    inlineFonts: false,
    preloadFonts: false,
    allowRules: [/\[data-theme=light\]/, /prefers-color-scheme:\s*light/],
  });
  const out = await beasties.process(shell);
  const css = [...out.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join('');
  if (!css) throw new Error('Beasties produced no critical CSS');
  return css;
}

function applyCritical(html, css, attr) {
  if (html.includes(attr)) return html;
  const links = blockingStylesheets(html);
  if (links.length === 0) return html;
  let out = html;
  for (const { tag } of links) out = out.replace(tag, '');
  const style = `<style ${attr}>${css}</style>`;
  if (out.includes('<head>')) out = out.replace('<head>', `<head>${style}`);
  else out = out.replace(/<head[^>]*>/, (h) => `${h}${style}`);
  const loaders = links.map((l) => ASYNC_CSS(l.href)).join('');
  return out.replace('</head>', `${loaders}</head>`);
}

function ecStylesheet(clientDir) {
  const name = readdirSync(join(clientDir, '_astro')).find((n) => n.startsWith('ec.') && n.endsWith('.css'));
  return name ? `/_astro/${name}` : '';
}

/**
 * @param {string} clientDir absolute path to dist/client
 * @param {{ info?: (m: string) => void }} [logger]
 */
export async function applyPageCriticalCss(clientDir, logger = console) {
  const ecHref = ecStylesheet(clientDir);
  const docsSample = readFileSync(join(clientDir, 'docs/launcher/notes/index.html'), 'utf8');
  const docsShellHtml = docsShell(docsSample, ecHref);
  if (!docsShellHtml) throw new Error('Could not build docs critical-CSS shell');
  const docsCss = await extractCritical(clientDir, docsShellHtml);
  logger.info?.(`[critical-css] docs critical ${docsCss.length} bytes`);

  const docFiles = [...walkHtml(join(clientDir, 'docs')), ...walkHtml(join(clientDir, 'es/docs'))];
  for (const file of docFiles) {
    writeFileSync(file, applyCritical(readFileSync(file, 'utf8'), docsCss, 'data-docs-critical'));
  }
  logger.info?.(`[critical-css] updated ${docFiles.length} docs pages`);

  const convertSample = readFileSync(join(clientDir, 'convert/heic-to-jpg/index.html'), 'utf8');
  const convertShellHtml = convertShell(convertSample);
  if (!convertShellHtml) throw new Error('Could not build convert critical-CSS shell');
  const convertCss = await extractCritical(clientDir, convertShellHtml);
  logger.info?.(`[critical-css] convert critical ${convertCss.length} bytes`);
  const convertFiles = walkHtml(join(clientDir, 'convert'));
  for (const file of convertFiles) {
    writeFileSync(file, applyCritical(readFileSync(file, 'utf8'), convertCss, 'data-convert-critical'));
  }
  logger.info?.(`[critical-css] updated ${convertFiles.length} convert pages`);
}
