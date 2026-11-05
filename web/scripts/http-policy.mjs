/** Pure HTTP policy for production-server.mjs: CSP, cache headers, canonical-host redirect. */

/** Report-Only CSP: external origins the site actually loads (GTM / GA4, Pagefind wasm). Fonts and media are self-hosted. */
export const CSP_REPORT_ONLY = [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://www.googletagmanager.com",
  "connect-src 'self' https://www.google-analytics.com https://region1.google-analytics.com https://*.analytics.google.com https://www.googletagmanager.com https://api.frankfurter.dev",
  "img-src 'self' data: https://www.googletagmanager.com https://www.google-analytics.com",
  "style-src 'self' 'unsafe-inline'",
  "font-src 'self'",
  "media-src 'self'",
  'frame-src https://www.googletagmanager.com',
  "base-uri 'self'",
  "form-action 'self'",
  'report-uri /api/csp-report/',
  'report-to csp',
].join('; ');

/** Cache-Control for a request path, or null to leave the header unset. */
export function cacheControlFor(path) {
  if (path.startsWith('/api/')) return 'no-store';
  if (path.startsWith('/_astro/')) return 'public, max-age=31536000, immutable';
  if (/^\/(?:og-welcome\.jpg|favicon\.ico|favicon\.png|icon\.png)$/.test(path) || path.startsWith('/og/')) {
    return 'public, max-age=604800';
  }
  if (path.startsWith('/img/') || path.startsWith('/video/')) return 'public, max-age=604800';
  if (path.endsWith('/') || path.endsWith('.html') || !path.split('/').pop()?.includes('.')) return 'no-cache';
  return null;
}

export const CANONICAL_HOST = 'ferrocast.app';
export const WWW_HOST = 'www.ferrocast.app';

/** Location for a 301 to the apex host, only when the Host header is exactly www.ferrocast.app (any port ignored). */
export function wwwRedirectLocation(host, url) {
  const h = String(host ?? '').toLowerCase().replace(/:\d+$/, '');
  if (h !== WWW_HOST) return null;
  const target = url && url.startsWith('/') && !url.startsWith('//') ? url : '/';
  return `https://${CANONICAL_HOST}${target}`;
}
