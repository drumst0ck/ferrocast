import { summaryAccessGranted } from './csp-report-auth.mjs';
import { ingestReport, summaryJson } from './csp-report-store.mjs';

const MAX_BODY = 16 * 1024;
const RATE_WINDOW_MS = 60_000;
const RATE_MAX = 120;

/** @type {Map<string, { count: number, reset: number }>} */
const rateByIp = new Map();

function clientIp(req) {
  const fwd = req.headers['x-forwarded-for'];
  if (typeof fwd === 'string' && fwd.length) return fwd.split(',')[0].trim();
  return req.socket?.remoteAddress ?? 'unknown';
}

function rateLimited(ip) {
  const now = Date.now();
  let bucket = rateByIp.get(ip);
  if (!bucket || now > bucket.reset) {
    bucket = { count: 0, reset: now + RATE_WINDOW_MS };
    rateByIp.set(ip, bucket);
  }
  bucket.count += 1;
  return bucket.count > RATE_MAX;
}

/** @param {unknown} body */
function extractReports(body) {
  /** @type {Record<string, unknown>[]} */
  const out = [];
  if (!body || typeof body !== 'object') return out;
  if ('csp-report' in body && body['csp-report'] && typeof body['csp-report'] === 'object') {
    out.push(/** @type {Record<string, unknown>} */ (body['csp-report']));
    return out;
  }
  if (Array.isArray(body)) {
    for (const item of body) {
      if (!item || typeof item !== 'object') continue;
      const row = /** @type {Record<string, unknown>} */ (item);
      if (row.body && typeof row.body === 'object') {
        const b = /** @type {Record<string, unknown>} */ (row.body);
        if (b['csp-report'] && typeof b['csp-report'] === 'object') {
          out.push(/** @type {Record<string, unknown>} */ (b['csp-report']));
        } else {
          out.push(b);
        }
      }
    }
    return out;
  }
  return out;
}

/**
 * @returns {boolean} true if handled
 */
export function handleCspApi(req, res, pathname) {
  if (pathname === '/api/csp-report/summary/' || pathname === '/api/csp-report/summary') {
    const url = new URL(req.url ?? '/', 'http://127.0.0.1');
    const allowed =
      req.method === 'GET' &&
      summaryAccessGranted(req.headers.authorization, url.searchParams.get('token'));
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('X-Robots-Tag', 'noindex');
    if (!allowed) {
      res.statusCode = 404;
      res.end();
      return true;
    }
    res.setHeader('Content-Type', 'application/json; charset=utf-8');
    res.statusCode = 200;
    res.end(JSON.stringify(summaryJson()));
    return true;
  }

  if (pathname !== '/api/csp-report/' && pathname !== '/api/csp-report') return false;

  if (req.method !== 'POST') {
    res.statusCode = 405;
    res.end();
    return true;
  }

  const ip = clientIp(req);
  if (rateLimited(ip)) {
    res.statusCode = 429;
    res.end();
    return true;
  }

  const ctype = String(req.headers['content-type'] ?? '').split(';')[0].trim().toLowerCase();
  if (ctype !== 'application/csp-report' && ctype !== 'application/reports+json') {
    res.statusCode = 415;
    res.end();
    return true;
  }

  /** @type {Buffer[]} */
  const chunks = [];
  let size = 0;
  req.on('data', (chunk) => {
    size += chunk.length;
    if (size > MAX_BODY) {
      req.destroy();
      return;
    }
    chunks.push(chunk);
  });
  req.on('end', () => {
    try {
      const raw = Buffer.concat(chunks).toString('utf8');
      const parsed = raw ? JSON.parse(raw) : null;
      for (const report of extractReports(parsed)) {
        const key = ingestReport(report);
        if (key) {
          console.log(
            JSON.stringify({
              type: 'csp-violation',
              key,
              effectiveDirective: report['effective-directive'] ?? report.effectiveDirective,
              blockedOrigin: report['blocked-uri'] ?? report.blockedURL,
              documentPath: report['document-uri'] ?? report.documentURL,
            }),
          );
        }
      }
    } catch {
      // ignore malformed bodies
    }
    res.statusCode = 204;
    res.end();
  });
  req.on('error', () => {
    res.statusCode = 204;
    res.end();
  });
  return true;
}
