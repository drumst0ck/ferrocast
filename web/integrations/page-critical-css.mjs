import { fileURLToPath } from 'node:url';
import { applyPageCriticalCss } from '../scripts/critical-css.mjs';

/** @returns {import('astro').AstroIntegration} */
export function pageCriticalCss() {
  return {
    name: 'page-critical-css',
    hooks: {
      'astro:build:done': async ({ dir, logger }) => {
        await applyPageCriticalCss(fileURLToPath(dir), logger);
      },
    },
  };
}
