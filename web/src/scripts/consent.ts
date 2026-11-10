import { initConsent, setConsent, type Choice, type ConsentEnv } from '../lib/consent/consent';

const env: ConsentEnv = {
  storage: (() => {
    try {
      return window.localStorage;
    } catch {
      return null;
    }
  })(),
  loadScript(src) {
    const s = document.createElement('script');
    s.async = true;
    s.src = src;
    document.head.appendChild(s);
  },
  dataLayer: ((window as unknown as { dataLayer?: unknown[] }).dataLayer ??= []),
  hostname: location.hostname,
  cookieNames: () => document.cookie.split(';').map((c) => c.split('=')[0].trim()).filter(Boolean),
  expireCookie(name, domain) {
    document.cookie = `${name}=; Max-Age=0; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/${domain ? `; domain=${domain}` : ''}`;
  },
};

const banner = document.getElementById('fc-consent');
let opener: HTMLElement | null = null;

function show(focus: boolean) {
  if (!banner) return;
  banner.hidden = false;
  if (focus) banner.focus();
}

function hide() {
  if (!banner) return;
  banner.hidden = true;
  if (opener) {
    opener.focus();
    opener = null;
  }
}

if (initConsent(env) === null) show(false);

banner?.addEventListener('click', (e) => {
  const btn = (e.target as Element).closest<HTMLElement>('[data-consent]');
  if (!btn) return;
  setConsent(env, btn.dataset.consent as Choice);
  hide();
});

document.addEventListener('click', (e) => {
  const link = (e.target as Element).closest<HTMLElement>('[data-cookie-settings]');
  if (!link || !banner) return;
  e.preventDefault();
  opener = link;
  show(true);
});
