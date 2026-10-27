import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyPageCriticalCss } from './critical-css.mjs';

const clientDir = join(dirname(fileURLToPath(import.meta.url)), '../dist/client');
await applyPageCriticalCss(clientDir);
