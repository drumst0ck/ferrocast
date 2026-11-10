import { describe, expect, it } from 'vitest';
import { CSP_HEADER, buildCsp, cacheControlFor, securityHeaders, wwwRedirectLocation } from './http-policy.mjs';

const CSP = buildCsp(); // same policy shape without hashes
const directive = (name) => CSP.split('; ').find((d) => d.startsWith(name + ' ')) ?? '';

describe('CSP hosts', () => {
  it('allows Pagefind wasm and GA4 analytics hosts', () => {
    expect(directive('script-src')).toContain("'wasm-unsafe-eval'");
    expect(directive('connect-src')).toContain('https://*.analytics.google.com');
  });
});

describe('cacheControlFor', () => {
  it('caches social cards (og-welcome.jpg and /og/*.png) for a week', () => {
    expect(cacheControlFor('/og-welcome.jpg')).toBe('public, max-age=604800');
    expect(cacheControlFor('/og/docs-home.png')).toBe('public, max-age=604800');
  });
  it('keeps the other rules', () => {
    expect(cacheControlFor('/og.jpg')).toBeNull();
    expect(cacheControlFor('/_astro/a.js')).toContain('immutable');
    expect(cacheControlFor('/api/x/')).toBe('no-store');
    expect(cacheControlFor('/docs/')).toBe('no-cache');
    expect(cacheControlFor('/img/a.png')).toBe('public, max-age=604800');
  });
});

describe('wwwRedirectLocation', () => {
  it('redirects www.ferrocast.app to the apex for any path, keeping the query', () => {
    expect(wwwRedirectLocation('www.ferrocast.app', '/docs/?q=1')).toBe('https://ferrocast.app/docs/?q=1');
    expect(wwwRedirectLocation('WWW.ferrocast.app:443', '/')).toBe('https://ferrocast.app/');
  });
  it('never redirects an arbitrary Host header (no open redirect)', () => {
    expect(wwwRedirectLocation('www.evil.com', '/changelog/')).toBeNull();
    expect(wwwRedirectLocation('www.ferrocast.app.evil.com', '/')).toBeNull();
    expect(wwwRedirectLocation('ferrocast.app', '/')).toBeNull();
    expect(wwwRedirectLocation(undefined, '/')).toBeNull();
  });
  it('does not turn a protocol-relative path into another host', () => {
    expect(wwwRedirectLocation('www.ferrocast.app', '//evil.com/x')).toBe('https://ferrocast.app/');
  });
});

describe('enforced CSP', () => {
  it('is sent under the Content-Security-Policy header name, never Report-Only', () => {
    expect(CSP_HEADER).toBe('Content-Security-Policy');
    const h = securityHeaders(buildCsp(["'sha256-abc='"]));
    expect(Object.keys(h)).toContain('Content-Security-Policy');
    expect(Object.keys(h).some((k) => /report-only/i.test(k))).toBe(false);
    expect(h['Content-Security-Policy']).toContain("frame-ancestors 'self'");
  });
  it('keeps violation reporting on the existing endpoint', () => {
    const csp = buildCsp();
    expect(csp).toContain('report-uri /api/csp-report/');
    expect(csp).toContain('report-to csp');
    expect(securityHeaders(csp)['Reporting-Endpoints']).toBe('csp="/api/csp-report/"');
  });
  it('allows the home demo API, GTM hosts and Pagefind wasm', () => {
    expect(directive('connect-src')).toContain('https://api.frankfurter.dev');
    expect(directive('script-src')).toContain('https://www.googletagmanager.com');
    expect(directive('script-src')).toContain("'wasm-unsafe-eval'");
  });
  it('lists inline script hashes instead of unsafe-inline when it has them', () => {
    expect(directive('script-src')).toContain("'unsafe-inline'");
    const s = buildCsp(["'sha256-AAA='", "'sha256-BBB='"]).split('; ').find((d) => d.startsWith('script-src'));
    expect(s).toContain("'sha256-AAA='");
    expect(s).not.toContain('unsafe-inline');
  });
});
