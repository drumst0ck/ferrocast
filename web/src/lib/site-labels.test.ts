import { describe, expect, it } from 'vitest';
import { homeHref, siteLabels } from './site-labels';
import { es as homeEs } from '../i18n/home.es';

describe('site labels', () => {
  it('has a Spanish accessible name for every shared label', () => {
    for (const k of Object.keys(siteLabels.en) as (keyof typeof siteLabels.en)[]) {
      expect(siteLabels.es[k], k).toBeTruthy();
      expect(siteLabels.es[k], k).not.toBe(siteLabels.en[k]);
    }
  });
  it('matches the home copy navigation label', () => {
    expect(siteLabels.es.mainNav).toBe(homeEs.nav.main);
  });
  it('links home inside the current language', () => {
    expect(homeHref('en')).toBe('/');
    expect(homeHref('es')).toBe('/es/');
  });
});
