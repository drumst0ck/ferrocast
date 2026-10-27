import { afterEach, describe, expect, it } from 'vitest';
import { summaryAccessGranted } from './csp-report-auth.mjs';

const KEY = 'CSP_REPORT_SUMMARY_TOKEN';

describe('summaryAccessGranted', () => {
  afterEach(() => {
    delete process.env[KEY];
  });

  it('allows anyone when the env var is unset or empty', () => {
    delete process.env[KEY];
    expect(summaryAccessGranted(undefined, null)).toBe(true);
    process.env[KEY] = '';
    expect(summaryAccessGranted(undefined, null)).toBe(true);
    process.env[KEY] = '   ';
    expect(summaryAccessGranted(undefined, null)).toBe(true);
  });

  it('denies a missing or wrong token', () => {
    process.env[KEY] = 'secret';
    expect(summaryAccessGranted(undefined, null)).toBe(false);
    expect(summaryAccessGranted('Bearer no', null)).toBe(false);
    expect(summaryAccessGranted(undefined, 'no')).toBe(false);
  });

  it('allows Bearer or query token', () => {
    process.env[KEY] = 'secret';
    expect(summaryAccessGranted('Bearer secret', null)).toBe(true);
    expect(summaryAccessGranted(undefined, 'secret')).toBe(true);
  });
});
