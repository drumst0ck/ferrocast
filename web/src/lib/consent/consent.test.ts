import { describe, expect, it } from 'vitest';
import { CONSENT_KEY, CONSENT_VERSION, GTM_ID, initConsent, setConsent, readConsent, type ConsentEnv } from './consent';

function fakeEnv(stored?: string) {
  const store = new Map<string, string>();
  if (stored !== undefined) store.set(CONSENT_KEY, stored);
  const scripts: string[] = [];
  const expired: string[] = [];
  const dataLayer: unknown[] = [];
  const env: ConsentEnv = {
    storage: {
      getItem: (k) => store.get(k) ?? null,
      setItem: (k, v) => void store.set(k, v),
    },
    loadScript: (src) => void scripts.push(src),
    dataLayer,
    hostname: 'ferrocast.app',
    cookieNames: () => ['_ga', '_ga_ABC123', 'session', '_gid', 'other_ga'],
    expireCookie: (name, domain) => void expired.push(`${name}@${domain ?? 'host'}`),
  };
  return { env, store, scripts, expired, dataLayer };
}

describe('consent before any choice', () => {
  it('loads nothing and asks (returns null) when nothing is stored', () => {
    const t = fakeEnv();
    expect(initConsent(t.env)).toBeNull();
    expect(t.scripts).toEqual([]);
    expect(t.dataLayer).toEqual([]);
  });
  it('asks again when the stored choice has an older policy version', () => {
    const t = fakeEnv(JSON.stringify({ v: CONSENT_VERSION - 1, choice: 'granted' }));
    expect(initConsent(t.env)).toBeNull();
    expect(t.scripts).toEqual([]);
  });
  it('treats corrupt storage as no choice', () => {
    const t = fakeEnv('{nope');
    expect(readConsent(t.env.storage)).toBeNull();
    expect(initConsent(t.env)).toBeNull();
  });
  it('survives a missing storage (private mode) without loading anything', () => {
    const t = fakeEnv();
    t.env.storage = null;
    expect(initConsent(t.env)).toBeNull();
    setConsent(t.env, 'denied');
    expect(t.scripts).toEqual([]);
  });
});

describe('accepting', () => {
  it('persists a versioned choice, sets Consent Mode v2 defaults denied then grants analytics only, and loads GTM once', () => {
    const t = fakeEnv();
    setConsent(t.env, 'granted');
    setConsent(t.env, 'granted');
    expect(JSON.parse(t.store.get(CONSENT_KEY)!)).toEqual({ v: CONSENT_VERSION, choice: 'granted' });
    expect(t.scripts).toEqual([`https://www.googletagmanager.com/gtm.js?id=${GTM_ID}`]);
    const calls = t.dataLayer.map((a) => Array.from(a as ArrayLike<unknown>));
    expect(calls[0].slice(0, 2)).toEqual(['consent', 'default']);
    expect(calls[0][2]).toMatchObject({ ad_storage: 'denied', ad_user_data: 'denied', ad_personalization: 'denied', analytics_storage: 'denied' });
    const update = calls.find((c) => c[1] === 'update')!;
    expect(update[2]).toEqual({ analytics_storage: 'granted' });
    expect(calls.findIndex((c) => c[1] === 'default')).toBeLessThan(calls.findIndex((c) => c[1] === 'update'));
  });
  it('loads GTM on the next page when the stored choice is granted', () => {
    const t = fakeEnv(JSON.stringify({ v: CONSENT_VERSION, choice: 'granted' }));
    expect(initConsent(t.env)).toBe('granted');
    expect(t.scripts).toHaveLength(1);
  });
});

describe('rejecting and withdrawing', () => {
  it('reject stores the choice, loads nothing and removes any _ga cookies', () => {
    const t = fakeEnv();
    setConsent(t.env, 'denied');
    expect(JSON.parse(t.store.get(CONSENT_KEY)!).choice).toBe('denied');
    expect(t.scripts).toEqual([]);
    expect(t.expired).toContain('_ga@host');
    expect(t.expired).toContain('_ga_ABC123@ferrocast.app');
    expect(t.expired).toContain('_ga@.ferrocast.app');
    expect(t.expired).toContain('_gid@host');
    expect(t.expired.some((e) => e.startsWith('session@') || e.startsWith('other_ga@'))).toBe(false);
  });
  it('stored denied loads nothing on the next page', () => {
    const t = fakeEnv(JSON.stringify({ v: CONSENT_VERSION, choice: 'denied' }));
    expect(initConsent(t.env)).toBe('denied');
    expect(t.scripts).toEqual([]);
  });
  it('withdrawing after GTM already loaded tells it analytics storage is denied again', () => {
    const t = fakeEnv();
    setConsent(t.env, 'granted');
    setConsent(t.env, 'denied');
    const last = Array.from(t.dataLayer[t.dataLayer.length - 1] as ArrayLike<unknown>);
    expect(last.slice(0, 2)).toEqual(['consent', 'update']);
    expect(last[2]).toEqual({ analytics_storage: 'denied' });
    expect(t.expired).toContain('_ga@host');
  });
});
