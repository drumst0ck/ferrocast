/**
 * Generate src/styles/home-critical.css from above-the-fold shell + source styles (Beasties).
 */
import Beasties from 'beasties';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const shell = readFileSync(join(webRoot, 'scripts/home-critical-shell.html'), 'utf8');

const beasties = new Beasties({
  path: webRoot,
  publicPath: '/',
  external: true,
  inlineFonts: true,
  preloadFonts: false,
  mergeStylesheets: true,
  compress: true,
  logLevel: 'warn',
});

const out = await beasties.process(shell);
const m = out.match(/<style[^>]*>([\s\S]*?)<\/style>/);
if (!m) throw new Error('Beasties did not produce critical CSS');
const css = m[1].trim();
writeFileSync(join(webRoot, 'src/styles/home-critical.css'), `${css}\n`);
console.log(`[generate-home-critical] Wrote ${css.length} bytes to src/styles/home-critical.css`);
