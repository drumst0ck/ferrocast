import { EventEmitter } from 'node:events';
import { beforeEach, describe, expect, it } from 'vitest';
import { handleCspApi, rateTableSize, resetRateLimitForTests } from './csp-report-handle.mjs';
import { resetAggregatesForTests, summaryJson } from './csp-report-store.mjs';

function post(body, { xff, ip = '203.0.113.9', doc = 'https://ferrocast.app/' } = {}) {
  const req = new EventEmitter();
  req.method = 'POST';
  req.url = '/api/csp-report/';
  req.headers = { 'content-type': 'application/csp-report' };
  if (xff) req.headers['x-forwarded-for'] = xff;
  req.socket = { remoteAddress: ip };
  const res = { statusCode: 0, headers: {}, setHeader() {}, end() {} };
  handleCspApi(req, res, '/api/csp-report/');
  const payload = body ?? { 'csp-report': { 'effective-directive': 'script-src', 'blocked-uri': 'inline', 'document-uri': doc } };
  req.emit('data', Buffer.from(JSON.stringify(payload)));
  req.emit('end');
  return res.statusCode;
}

describe('csp report endpoint', () => {
  beforeEach(() => {
    resetAggregatesForTests();
    resetRateLimitForTests();
  });

  it('rate limits per real client, not per forged X-Forwarded-For prefix', () => {
    const codes = [];
    for (let i = 1; i <= 125; i++) codes.push(post(undefined, { xff: `10.0.0.${i}, 198.51.100.7` }));
    expect(codes.filter((c) => c === 204)).toHaveLength(120);
    expect(codes.filter((c) => c === 429)).toHaveLength(5);
  });

  it('keeps the rate-limit table bounded', () => {
    for (let i = 0; i < 30_000; i++) post(undefined, { ip: `192.0.${i >> 8}.${i & 255}` });
    expect(rateTableSize()).toBeLessThanOrEqual(10_000);
    // a new client while the table is full is limited rather than stored
    expect(post(undefined, { ip: '198.51.100.250' })).toBe(429);
    expect(rateTableSize()).toBeLessThanOrEqual(10_000);
  });

  it('records only reports about our own site', () => {
    post(undefined, { doc: 'https://evil.example/' });
    expect(summaryJson().violations).toHaveLength(0);
    post(undefined, { doc: 'https://ferrocast.app/docs/' });
    expect(summaryJson().violations).toHaveLength(1);
  });
});
