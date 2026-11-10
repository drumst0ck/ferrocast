/** Inline-script hashing for the enforced CSP: every executable inline <script> in built HTML gets a sha256 source. */
import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

const EXECUTABLE_TYPE = /^(?:|module|text\/javascript|application\/javascript)$/i;

/** sha256 CSP sources ('sha256-...') for the executable inline scripts of one HTML document. */
export function inlineScriptHashes(html) {
  const out = new Set();
  for (const m of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
    const attrs = m[1];
    if (/\bsrc\s*=/i.test(attrs)) continue;
    const type = /\btype\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/i.exec(attrs);
    if (!EXECUTABLE_TYPE.test((type?.[1] ?? type?.[2] ?? type?.[3] ?? '').trim())) continue;
    out.add(`'sha256-${createHash('sha256').update(m[2], 'utf8').digest('base64')}'`);
  }
  return out;
}

/** sha256 sources for inline event-handler attributes (e.g. the async-stylesheet onload), for script-src-attr 'unsafe-hashes'. */
export function inlineHandlerHashes(html) {
  const out = new Set();
  const tags = html.replace(/<script\b[\s\S]*?<\/script>/gi, '').replace(/<style\b[\s\S]*?<\/style>/gi, '');
  for (const m of tags.matchAll(/<[a-z][^>]*?\s(on[a-z]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/gi)) {
    const body = (m[2] ?? m[3]).replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&amp;/g, '&');
    out.add(`'sha256-${createHash('sha256').update(body, 'utf8').digest('base64')}'`);
  }
  return out;
}

async function* htmlFiles(dir) {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* htmlFiles(p);
    else if (e.name.endsWith('.html')) yield p;
  }
}

/** Union of inline-script hashes across every built HTML page under dir (sorted, stable). */
export async function collectDistHashes(dir) {
  const all = new Set();
  for await (const f of htmlFiles(dir)) for (const h of inlineScriptHashes(await readFile(f, 'utf8'))) all.add(h);
  return [...all].sort();
}

/** Union of inline event-handler hashes across every built HTML page under dir. */
export async function collectDistHandlerHashes(dir) {
  const all = new Set();
  for await (const f of htmlFiles(dir)) for (const h of inlineHandlerHashes(await readFile(f, 'utf8'))) all.add(h);
  return [...all].sort();
}
