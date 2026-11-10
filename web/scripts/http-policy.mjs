/** Pure HTTP policy for production-server.mjs: CSP, cache headers, canonical-host redirect. */

/** The enforced policy is sent under this name (it used to be Content-Security-Policy-Report-Only). */
export const CSP_HEADER = 'Content-Security-Policy';

/**
 * Enforced CSP. `inlineScriptHashes` are the 'sha256-...' sources of every inline script in the built pages
 * (see csp-inline.mjs); without them inline scripts would be blocked, so an empty list falls back to
 * 'unsafe-inline'. `handlerHashes` allow the few inline event-handler attributes (async stylesheet onload) through
 * script-src-attr 'unsafe-hashes'; none means no inline handlers at all. Google hosts are only reached after the visitor accepts analytics (consent.ts).
 * Fonts and media are self-hosted; the home demo calls the Frankfurter API; Pagefind needs wasm.
 */
export function buildCsp(inlineScriptHashes = [], handlerHashes = []) {
  const inline = inlineScriptHashes.length ? inlineScriptHashes.join(' ') : "'unsafe-inline'";
  return [
    "default-src 'self'",
    `script-src 'self' ${inline} 'wasm-unsafe-eval' https://www.googletagmanager.com`,
    `script-src-attr ${handlerHashes.length ? `'unsafe-hashes' ${handlerHashes.join(' ')}` : "'none'"}`,
    "connect-src 'self' https://www.google-analytics.com https://*.google-analytics.com https://*.analytics.google.com https://www.googletagmanager.com https://api.frankfurter.dev",
    "img-src 'self' data: https://www.googletagmanager.com https://www.google-analytics.com https://*.google-analytics.com",
    "style-src 'self' 'unsafe-inline'",
    "font-src 'self'",
    "media-src 'self'",
    "frame-src 'none'",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "frame-ancestors 'self'",
    'report-uri /api/csp-report/',
    'report-to csp',
  ].join('; ');
}

/** Security headers for every response (HSTS, nosniff, referrer, enforced CSP, report endpoint). */
export function securityHeaders(csp) {
  return {
    'Strict-Transport-Security': 'max-age=31536000; includeSubDomains',
    'X-Content-Type-Options': 'nosniff',
    'Referrer-Policy': 'strict-origin-when-cross-origin',
    [CSP_HEADER]: csp,
    'Reporting-Endpoints': 'csp="/api/csp-report/"',
  };
}

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
