/** Analytics consent: nothing from Google loads or is stored until the visitor accepts. */
export const CONSENT_KEY = 'ferrocast-consent';
/** Bump when the cookie policy changes so every visitor is asked again. */
export const CONSENT_VERSION = 1;
export const GTM_ID = 'GTM-KC23GXFQ';
export type Choice = 'granted' | 'denied';

export interface ConsentEnv {
  storage: Pick<Storage, 'getItem' | 'setItem'> | null;
  loadScript: (src: string) => void;
  dataLayer: unknown[];
  hostname: string;
  cookieNames: () => string[];
  /** Expire one cookie; domain undefined means the current host only. */
  expireCookie: (name: string, domain?: string) => void;
}

const GA_COOKIE = /^(?:_ga|_ga_.+|_gid|_gat(?:_.+)?)$/;
const loaded = new WeakSet<unknown[]>();

export function readConsent(storage: ConsentEnv['storage']): Choice | null {
  try {
    const raw = storage?.getItem(CONSENT_KEY);
    if (!raw) return null;
    const v = JSON.parse(raw) as { v?: unknown; choice?: unknown };
    if (v.v !== CONSENT_VERSION) return null;
    return v.choice === 'granted' || v.choice === 'denied' ? v.choice : null;
  } catch {
    return null;
  }
}

function gtag(env: ConsentEnv, ...args: unknown[]) {
  // GTM reads the dataLayer as the Arguments object, not as an array.
  (function (..._a: unknown[]) {
    env.dataLayer.push(arguments);
  })(...args);
}

function loadGtm(env: ConsentEnv) {
  if (loaded.has(env.dataLayer)) return;
  loaded.add(env.dataLayer);
  gtag(env, 'consent', 'default', {
    ad_storage: 'denied',
    ad_user_data: 'denied',
    ad_personalization: 'denied',
    analytics_storage: 'denied',
    wait_for_update: 500,
  });
  gtag(env, 'consent', 'update', { analytics_storage: 'granted' });
  env.dataLayer.push({ 'gtm.start': Date.now(), event: 'gtm.js' });
  env.loadScript(`https://www.googletagmanager.com/gtm.js?id=${GTM_ID}`);
}

export function deleteGaCookies(env: ConsentEnv) {
  const parts = env.hostname.split('.');
  const domains = new Set<string | undefined>([undefined, env.hostname]);
  for (let i = 0; i < parts.length - 1; i++) domains.add('.' + parts.slice(i).join('.'));
  for (const name of env.cookieNames()) {
    if (!GA_COOKIE.test(name)) continue;
    for (const d of domains) env.expireCookie(name, d);
  }
}

/** Run once per page: applies a stored choice. Returns it, or null when the banner must be shown. */
export function initConsent(env: ConsentEnv): Choice | null {
  const choice = readConsent(env.storage);
  if (choice === 'granted') loadGtm(env);
  return choice;
}

/** Persist and apply the visitor's choice (Accept / Reject / withdraw). */
export function setConsent(env: ConsentEnv, choice: Choice) {
  try {
    env.storage?.setItem(CONSENT_KEY, JSON.stringify({ v: CONSENT_VERSION, choice }));
  } catch {
    /* storage blocked: the choice only lasts for this page */
  }
  if (choice === 'granted') {
    loadGtm(env);
    return;
  }
  if (loaded.has(env.dataLayer)) gtag(env, 'consent', 'update', { analytics_storage: 'denied' });
  deleteGaCookies(env);
}
