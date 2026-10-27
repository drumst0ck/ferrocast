/**
 * Production HTTP entry: Astro standalone handler plus security, CSP, and cache headers.
 * Dynamic-import entry.mjs after ASTRO_NODE_AUTOSTART=disabled (static imports run first).
 */
process.env.ASTRO_NODE_AUTOSTART = 'disabled';

import http from 'node:http';

const PORT = Number(process.env.PORT ?? 4321);
const HOST = process.env.HOST ?? '0.0.0.0';

/** Report-Only CSP: external origins the site actually loads (GTM / GA4). Fonts and media are self-hosted. */
const CSP_REPORT_ONLY = [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' https://www.googletagmanager.com",
  "connect-src 'self' https://www.google-analytics.com https://region1.google-analytics.com https://www.googletagmanager.com https://api.frankfurter.dev",
  "img-src 'self' data: https://www.googletagmanager.com https://www.google-analytics.com",
  "style-src 'self' 'unsafe-inline'",
  "font-src 'self'",
  "media-src 'self'",
  "frame-src https://www.googletagmanager.com",
  "base-uri 'self'",
  "form-action 'self'",
].join('; ');

function applyHeaders(req, res) {
  const path = (req.url ?? '/').split('?')[0];

  res.setHeader('Strict-Transport-Security', 'max-age=31536000; includeSubDomains');
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('Referrer-Policy', 'strict-origin-when-cross-origin');
  res.setHeader('Content-Security-Policy', "frame-ancestors 'self'");
  res.setHeader('Content-Security-Policy-Report-Only', CSP_REPORT_ONLY);

  if (path.startsWith('/_astro/')) {
    res.setHeader('Cache-Control', 'public, max-age=31536000, immutable');
  } else if (
    /^\/(?:og\.jpg|favicon\.ico|favicon\.png|icon\.png)$/.test(path)
  ) {
    res.setHeader('Cache-Control', 'public, max-age=604800');
  } else if (path.startsWith('/img/') || path.startsWith('/video/')) {
    res.setHeader('Cache-Control', 'public, max-age=604800');
  } else if (
    path.endsWith('/') ||
    path.endsWith('.html') ||
    !path.split('/').pop()?.includes('.')
  ) {
    res.setHeader('Cache-Control', 'no-cache');
  }
}

const { handler } = await import('../dist/server/entry.mjs');

const server = http.createServer((req, res) => {
  applyHeaders(req, res);
  handler(req, res);
});

server.listen(PORT, HOST, () => {
  console.log(`[production-server] listening on http://${HOST}:${PORT}`);
});
