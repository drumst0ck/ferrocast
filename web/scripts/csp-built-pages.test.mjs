import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { collectDistHandlerHashes, collectDistHashes, inlineHandlerHashes, inlineScriptHashes } from './csp-inline.mjs';
import { CSP_HEADER, buildCsp, securityHeaders } from './http-policy.mjs';

const clientDir = join(dirname(fileURLToPath(import.meta.url)), '../dist/client');

const scriptSrc = (csp) => csp.split('; ').find((d) => d.startsWith('script-src ')) ?? '';

/** Independent check: would a browser run this inline script under this policy? */
function allowedInline(csp, body) {
  const src = scriptSrc(csp);
  if (src.includes("'unsafe-inline'")) return true;
  return src.includes(`'sha256-${createHash('sha256').update(body, 'utf8').digest('base64')}'`);
}

function inlineScripts(html) {
  return [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)]
    .filter(([, attrs]) => !/\bsrc\s*=/i.test(attrs))
    .filter(([, attrs]) => !/type\s*=\s*"(?:application\/ld\+json|application\/json|importmap)"/i.test(attrs))
    .map(([, , body]) => body);
}

async function* htmlFiles(dir) {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* htmlFiles(p);
    else if (e.name.endsWith('.html')) yield p;
  }
}

describe('inlineScriptHashes', () => {
  it('hashes executable inline scripts, ignores external ones and data blocks', () => {
    const html = '<script>a()</script><script src="/x.js"></script><script type="application/ld+json">{}</script><script type="module">b()</script>';
    const hashes = [...inlineScriptHashes(html)];
    expect(hashes).toHaveLength(2);
    expect(hashes[0]).toBe(`'sha256-${createHash('sha256').update('a()').digest('base64')}'`);
  });
  it('a script that is not in the policy is not allowed', () => {
    const csp = buildCsp([...inlineScriptHashes('<script>ok()</script>')]);
    expect(allowedInline(csp, 'ok()')).toBe(true);
    expect(allowedInline(csp, 'evil()')).toBe(false);
  });
});

describe('inlineHandlerHashes', () => {
  it('hashes inline handler attributes only when listed in script-src-attr', () => {
    const html = `<link rel="stylesheet" media="print" onload="this.media='all'"><script>var x = "onclick=1"</script>`;
    const hashes = [...inlineHandlerHashes(html)];
    expect(hashes).toEqual([`'sha256-${createHash('sha256').update("this.media='all'").digest('base64')}'`]);
    expect(buildCsp([], hashes)).toContain(`script-src-attr 'unsafe-hashes' ${hashes[0]}`);
    expect(buildCsp([], [])).toContain("script-src-attr 'none'");
  });
});

describe.skipIf(!existsSync(join(clientDir, 'index.html')))('built pages under the enforced policy', () => {
  it('every inline script on every built page is allowed, and the header is Content-Security-Policy', async () => {
    const csp = buildCsp(await collectDistHashes(clientDir), await collectDistHandlerHashes(clientDir));
    const attrSrc = csp.split('; ').find((d) => d.startsWith('script-src-attr ')) ?? '';
    const headers = securityHeaders(csp);
    expect(Object.keys(headers)).toContain(CSP_HEADER);
    expect(CSP_HEADER).toBe('Content-Security-Policy');
    expect(scriptSrc(csp)).not.toContain('unsafe-inline');
    let pages = 0;
    const blocked = [];
    for await (const f of htmlFiles(clientDir)) {
      pages++;
      const html = await readFile(f, 'utf8');
      for (const body of inlineScripts(html)) if (!allowedInline(csp, body)) blocked.push(f);
      for (const m of html.replace(/<script\b[\s\S]*?<\/script>/gi, '').matchAll(/<[a-z][^>]*?\s(on[a-z]+)\s*=\s*"([^"]*)"/gi)) {
        const h = `'sha256-${createHash('sha256').update(m[2], 'utf8').digest('base64')}'`;
        if (!attrSrc.includes("'unsafe-hashes'") || !attrSrc.includes(h)) blocked.push(`${f} (inline handler ${m[1]})`);
      }
      if (/href\s*=\s*["']javascript:/i.test(html)) blocked.push(`${f} (javascript: url)`);
    }
    expect(pages).toBeGreaterThan(50);
    expect(blocked).toEqual([]);
  });

  it('no built page references Google in its markup (no GTM script tag, iframe or noscript pixel before consent)', async () => {
    const offenders = [];
    for await (const f of htmlFiles(clientDir)) {
      const html = await readFile(f, 'utf8');
      // the consent script may contain the GTM URL as a string; nothing may reference Google as a tag, frame, image or link
      const markup = html.replace(/<script\b(?![^>]*\bsrc=)[^>]*>[\s\S]*?<\/script>/gi, '');
      if (/googletagmanager\.com|google-analytics\.com/.test(markup)) offenders.push(f);
    }
    expect(offenders).toEqual([]);
  });

  it('serves the consent banner, footer settings link and cookie pages in both languages', async () => {
    for (const [file, settings, accept] of [
      ['cookies/index.html', 'Cookie settings', 'Accept'],
      ['es/cookies/index.html', 'Configuración de cookies', 'Aceptar'],
      ['index.html', 'Cookie settings', 'Accept'],
      ['es/index.html', 'Configuración de cookies', 'Aceptar'],
      ['docs/index.html', 'Cookie settings', 'Accept'],
      ['es/docs/index.html', 'Configuración de cookies', 'Aceptar'],
    ]) {
      const html = await readFile(join(clientDir, file), 'utf8');
      expect(html, file).toContain('id="fc-consent"');
      expect(html, file).toContain('data-cookie-settings');
      expect(html, file).toContain(settings);
      expect(html, file).toContain(`>${accept}<`);
      expect(html, file).toMatch(/data-consent="denied"[^>]*>[^<]+<\/button>\s*<button[^>]*data-consent="granted"/);
    }
  });

  it('puts the consent banner before any focusable element except skip links, so keyboard users reach it first', async () => {
    for (const file of ['index.html', 'es/index.html', 'docs/index.html', 'es/docs/index.html', 'cookies/index.html', 'es/cookies/index.html']) {
      const html = await readFile(join(clientDir, file), 'utf8');
      const before = html.slice(html.indexOf('<body'), html.indexOf('id="fc-consent"'));
      const focusable = [...before.matchAll(/<(?:a|button|input|select|textarea|summary)\b[^>]*>/gi)]
        .map(([tag]) => tag)
        .filter((tag) => !/\bhref="#/.test(tag));
      expect(focusable, file).toEqual([]);
    }
  });

  it('focuses the banner without scrolling when it first appears', async () => {
    const src = await readFile(join(dirname(fileURLToPath(import.meta.url)), '../src/scripts/consent.ts'), 'utf8');
    expect(src).toMatch(/if \(initConsent\(env\) === null\) show\(true\)/);
    expect(src).toContain('banner.focus({ preventScroll: true })');
  });
});
