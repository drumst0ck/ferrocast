import { readdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join, relative, sep } from 'node:path';
import { longTitles, MAX_TITLE } from '../src/lib/seo/title-length.ts';

async function* htmlFiles(dir) {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* htmlFiles(p);
    else if (e.name.endsWith('.html')) yield p;
  }
}

/** Warns (never fails the build) about non-docs pages whose <title> is over 60 characters. @returns {import('astro').AstroIntegration} */
export function titleLength() {
  return {
    name: 'title-length',
    hooks: {
      'astro:build:done': async ({ dir, logger }) => {
        try {
          const root = fileURLToPath(dir);
          const pages = [];
          for await (const f of htmlFiles(root)) {
            const path = '/' + relative(root, f).split(sep).join('/').replace(/index\.html$/, '');
            pages.push({ path, html: await readFile(f, 'utf8') });
          }
          const long = longTitles(pages);
          for (const t of long) logger.warn(`title over ${MAX_TITLE} chars (${t.length}): ${t.path} "${t.title}"`);
          if (long.length) logger.warn(`${long.length} non-docs page title(s) over ${MAX_TITLE} characters`);
        } catch (err) {
          logger.warn(`title-length check skipped: ${err}`);
        }
      },
    },
  };
}
