import { describe, expect, it } from 'vitest';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { serveHtmlFile } from './static-serve.mjs';

function fakeRes() {
  const headers = {};
  return {
    statusCode: 0, headers, body: undefined, ended: false,
    setHeader(k, v) { headers[k.toLowerCase()] = v; },
    end(b) { this.body = b; this.ended = true; },
  };
}
const dir = mkdtempSync(join(tmpdir(), 'ss-'));
const file = join(dir, 'index.html');
writeFileSync(file, '<h1>hi</h1>');

describe('serveHtmlFile', () => {
  it('serves GET with ETag and Content-Length', async () => {
    const res = fakeRes();
    expect(await serveHtmlFile({ method: 'GET', headers: {} }, res, file)).toBe(true);
    expect(res.statusCode).toBe(200);
    expect(res.headers.etag).toMatch(/^W\/"/);
    expect(res.headers['content-length']).toBe(String(Buffer.byteLength('<h1>hi</h1>')));
    expect(res.body.toString()).toBe('<h1>hi</h1>');
  });
  it('HEAD reports the real Content-Length without a body', async () => {
    const res = fakeRes();
    await serveHtmlFile({ method: 'HEAD', headers: {} }, res, file);
    expect(res.headers['content-length']).toBe('11');
    expect(res.body).toBeUndefined();
  });
  it('answers 304 when If-None-Match matches', async () => {
    const first = fakeRes();
    await serveHtmlFile({ method: 'GET', headers: {} }, first, file);
    const res = fakeRes();
    await serveHtmlFile({ method: 'GET', headers: { 'if-none-match': first.headers.etag } }, res, file);
    expect(res.statusCode).toBe(304);
    expect(res.body).toBeUndefined();
  });
  it('returns false when the file does not exist', async () => {
    expect(await serveHtmlFile({ method: 'GET', headers: {} }, fakeRes(), join(dir, 'nope.html'))).toBe(false);
  });
});
