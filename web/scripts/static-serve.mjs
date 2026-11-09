import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

/**
 * Serve a prerendered HTML file without blocking the event loop: weak ETag, 304 on If-None-Match,
 * and a real Content-Length for HEAD. Returns false when the file cannot be read.
 */
export async function serveHtmlFile(req, res, file) {
  let body;
  try {
    body = await readFile(file);
  } catch {
    return false;
  }
  const etag = `W/"${createHash('sha1').update(body).digest('base64url')}"`;
  res.setHeader('ETag', etag);
  const inm = req.headers['if-none-match'];
  if (inm && inm.split(',').some((t) => t.trim() === etag || t.trim() === '*')) {
    res.statusCode = 304;
    res.end();
    return true;
  }
  res.statusCode = 200;
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.setHeader('Content-Length', String(body.length));
  if (req.method === 'HEAD') {
    res.end();
    return true;
  }
  res.end(body);
  return true;
}
