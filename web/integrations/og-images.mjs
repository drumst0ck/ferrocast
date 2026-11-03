import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { renderOgImage } from '../src/lib/og/render.ts';
import { ogText } from '../src/lib/og/paths.ts';

const decode = (s) => s.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(+n)).replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCodePoint(parseInt(n, 16)));
const meta = (html, re) => html.match(re)?.[1];

/**
 * After the build, writes dist/og/<slug>.png for every page whose og:image points at /og/*.png (see lib/og/paths.ts and
 * layouts/Base.astro). The text is the page's og:title without the " | Ferrocast" suffix.
 * @returns {import('astro').AstroIntegration}
 */
export function ogImages() {
  return {
    name: 'og-images',
    hooks: {
      'astro:build:done': async ({ dir, pages, logger }) => {
        const root = fileURLToPath(dir);
        const started = Date.now();
        const jobs = new Map();
        for (const { pathname } of pages) {
          let html;
          try { html = await readFile(path.join(root, pathname, 'index.html'), 'utf8'); } catch { continue; }
          const img = meta(html, /<meta property="og:image" content="https?:\/\/[^/"]+(\/og\/[^"]+\.png)"/);
          const title = meta(html, /<meta property="og:title" content="([^"]*)"/);
          if (img && title) jobs.set(img, decode(title));
        }
        await mkdir(path.join(root, 'og'), { recursive: true });
        for (const [img, title] of jobs) await writeFile(path.join(root, img), await renderOgImage(ogText(title)));
        logger.info(`wrote ${jobs.size} og images in ${Date.now() - started} ms`);
      },
    },
  };
}
