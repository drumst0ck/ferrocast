/**
 * Post-build: one deferred home stylesheet + inlined critical CSS from src/styles/home-critical.css.
 */
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

const HOME_HTML = ['index.html', 'es/index.html'];
const ASYNC_CSS_LOADER =
  '<link rel="preload" href="HREF" as="style"><link rel="stylesheet" href="HREF" media="print" onload="this.media=\'all\'"><noscript><link rel="stylesheet" href="HREF"></noscript>';

/** @param {string} dir dist/client absolute path */
function mergeHomeStylesheets(dir, html) {
  const linkRe = /<link rel="stylesheet" href="(\/_astro\/index\.[^"]+\.css)">/g;
  const hrefs = [...html.matchAll(linkRe)].map((m) => m[1]);
  if (hrefs.length === 0) return html;

  const parts = hrefs.map((href) =>
    readFileSync(join(dir, href.replace(/^\//, '')), 'utf8'),
  );
  const merged = parts.join('\n');
  const hash = createHash('sha256').update(merged).digest('hex').slice(0, 8);
  const mergedHref = `/_astro/home.${hash}.css`;
  writeFileSync(join(dir, mergedHref.replace(/^\//, '')), merged);

  let out = html;
  for (const href of hrefs) {
    out = out.replace(`<link rel="stylesheet" href="${href}">`, '');
  }
  const loader = ASYNC_CSS_LOADER.replaceAll('HREF', mergedHref);
  return out.replace('</head>', `${loader}\n</head>`);
}

function injectCriticalStyle(html, criticalCss) {
  const tag = `<style data-home-critical>${criticalCss}</style>`;
  if (html.includes('data-home-critical')) return html;
  return html.replace('<head>', `<head>${tag}`);
}

/** @returns {import('astro').AstroIntegration} */
export function homeCriticalCss() {
  let criticalCss = '';
  try {
    criticalCss = readFileSync(join(webRoot, 'src/styles/home-critical.css'), 'utf8').trim();
  } catch {
    criticalCss = '';
  }

  return {
    name: 'home-critical-css',
    hooks: {
      'astro:build:done': async ({ dir, logger }) => {
        if (!criticalCss) {
          logger.warn('home-critical.css missing; run node scripts/generate-home-critical.mjs');
          return;
        }
        const clientDir = fileURLToPath(dir);
        for (const rel of HOME_HTML) {
          const file = join(clientDir, rel);
          let html;
          try {
            html = readFileSync(file, 'utf8');
          } catch {
            continue;
          }
          if (!html.includes('rel="stylesheet"')) continue;
          html = mergeHomeStylesheets(clientDir, html);
          html = injectCriticalStyle(html, criticalCss);
          writeFileSync(file, html);
          logger.info(`Deferred CSS + critical inline applied to ${rel}`);
        }
      },
    },
  };
}
