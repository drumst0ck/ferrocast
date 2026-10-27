import { describe, expect, it, beforeEach } from 'vitest';
import {
  ingestReport,
  resetAggregatesForTests,
  shouldIgnoreReport,
  summaryJson,
} from './csp-report-store.mjs';

describe('csp-report-store', () => {
  beforeEach(() => resetAggregatesForTests());

  it('ignores extension schemes', () => {
    expect(
      shouldIgnoreReport({
        'blocked-uri': 'chrome-extension://abc/script.js',
        'document-uri': 'https://ferrocast.app/',
      }),
    ).toBe(true);
  });

  it('aggregates by directive, origin, and document path', () => {
    ingestReport({
      'effective-directive': 'connect-src',
      'blocked-uri': 'https://evil.example.com/track?secret=1',
      'document-uri': 'https://ferrocast.app/',
    });
    ingestReport({
      'effective-directive': 'connect-src',
      'blocked-uri': 'https://evil.example.com/other',
      'document-uri': 'https://ferrocast.app/',
    });
    const s = summaryJson();
    expect(s.violations).toHaveLength(1);
    expect(s.violations[0].count).toBe(2);
    expect(s.violations[0].blockedOrigin).toBe('https://evil.example.com');
    expect(s.violations[0].documentPath).toBe('/');
  });

  it('counts inline violations from our document', () => {
    ingestReport({
      'effective-directive': 'script-src',
      'blocked-uri': 'inline',
      'document-uri': 'https://ferrocast.app/',
      'source-file': 'https://ferrocast.app/',
    });
    expect(summaryJson().violations[0].blockedOrigin).toBe('inline');
  });
});
