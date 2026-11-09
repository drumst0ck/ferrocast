/**
 * Production HTTP entry: Astro standalone handler plus security, CSP, and cache headers.
 * Dynamic-import entry.mjs after ASTRO_NODE_AUTOSTART=disabled (static imports run first).
 */
process.env.ASTRO_NODE_AUTOSTART = 'disabled';

import http from 'node:http';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { handleCspApi } from './csp-report-handle.mjs';
import { serveHtmlFile } from './static-serve.mjs';
import { CSP_REPORT_ONLY, cacheControlFor, wwwRedirectLocation } from './http-policy.mjs';

const PORT = Number(process.env.PORT ?? 4321);
const HOST = process.env.HOST ?? '0.0.0.0';
const webRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const clientDir = join(webRoot, 'dist/client');

const HOME_FILES = new Map([
  ['/', 'index.html'],
  ['/index.html', 'index.html'],
  ['/es/', 'es/index.html'],
  ['/es/index.html', 'es/index.html'],
]);

function applyHeaders(req, res) {
  const path = (req.url ?? '/').split('?')[0];

  res.setHeader('Strict-Transport-Security', 'max-age=31536000; includeSubDomains');
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('Referrer-Policy', 'strict-origin-when-cross-origin');
  res.setHeader('Content-Security-Policy', "frame-ancestors 'self'");
  res.setHeader('Content-Security-Policy-Report-Only', CSP_REPORT_ONLY);
  res.setHeader('Reporting-Endpoints', 'csp="/api/csp-report/"');

  const cache = cacheControlFor(path);
  if (cache) res.setHeader('Cache-Control', cache);
}

function prerenderedHtmlFile(urlPath) {
  const mapped = HOME_FILES.get(urlPath);
  if (mapped) return join(clientDir, mapped);
  const staticPrefix =
    urlPath.startsWith('/docs/') ||
    urlPath.startsWith('/es/docs/') ||
    urlPath.startsWith('/convert/');
  if (!staticPrefix || !urlPath.endsWith('/') || urlPath.includes('..')) return null;
  let decoded;
  try {
    decoded = decodeURIComponent(urlPath);
  } catch {
    return null;
  }
  if (decoded.includes('..')) return null;
  const abs = resolve(clientDir, decoded.slice(1) + 'index.html');
  const root = resolve(clientDir) + sep;
  if (!abs.startsWith(root)) return null;
  return abs;
}

async function tryServePrerendered(req, res) {
  if (req.method !== 'GET' && req.method !== 'HEAD') return false;
  const path = (req.url ?? '/').split('?')[0];
  const file = prerenderedHtmlFile(path);
  if (!file) return false;
  return serveHtmlFile(req, res, file);
}

const { handler } = await import('../dist/server/entry.mjs');

const server = http.createServer(async (req, res) => {
  const path = (req.url ?? '/').split('?')[0];
  const www = wwwRedirectLocation(req.headers.host, req.url);
  if (www) {
    res.statusCode = 301;
    res.setHeader('Location', www);
    res.setHeader('Cache-Control', 'public, max-age=3600');
    res.end();
    return;
  }
  applyHeaders(req, res);
  if (handleCspApi(req, res, path)) return;
  try {
    if (await tryServePrerendered(req, res)) return;
  } catch (err) {
    console.error('[production-server] static serve failed', err);
    if (!res.headersSent) {
      res.statusCode = 500;
      res.end();
    }
    return;
  }
  handler(req, res);
});

server.listen(PORT, HOST, () => {
  console.log(`[production-server] listening on http://${HOST}:${PORT}`);
});
