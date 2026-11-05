import { describe, expect, it } from 'vitest';
import { CSP_REPORT_ONLY, cacheControlFor, wwwRedirectLocation } from './http-policy.mjs';

const directive = (name) => CSP_REPORT_ONLY.split('; ').find((d) => d.startsWith(name + ' ')) ?? '';

describe('report-only CSP', () => {
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
