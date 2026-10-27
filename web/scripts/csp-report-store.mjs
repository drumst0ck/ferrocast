/** In-memory CSP violation aggregates for the current process (production-server). */

const EXT_SCHEMES = /^(chrome-extension|moz-extension|safari-web-extension):/i;
const MAX_KEYS = 500;

/** @type {Map<string, { count: number, firstSeen: string, lastSeen: string, effectiveDirective: string, blockedOrigin: string, documentPath: string }>} */
const aggregates = new Map();
export const processStartedAt = new Date().toISOString();

function documentPath(url) {
  if (!url) return '/';
  try {
    const p = new URL(url).pathname || '/';
    return p.endsWith('/') || p.includes('.') ? p : `${p}/`;
  } catch {
    return '/';
  }
}

function blockedOrigin(raw) {
  if (!raw || raw === 'inline' || raw === 'eval') return raw || 'unknown';
  try {
    return new URL(raw).origin;
  } catch {
    return raw.split('?')[0].slice(0, 120);
  }
}

/** @param {Record<string, unknown>} report */
export function shouldIgnoreReport(report) {
  const blocked = String(report['blocked-uri'] ?? report.blockedURL ?? '');
  const source = String(report['source-file'] ?? report.sourceFile ?? '');
  if (EXT_SCHEMES.test(blocked) || EXT_SCHEMES.test(source)) return true;
  const doc = String(report['document-uri'] ?? report.documentURL ?? '');
  if (EXT_SCHEMES.test(doc)) return true;
  if ((blocked === 'inline' || blocked === 'eval') && EXT_SCHEMES.test(source)) return true;
  return false;
}

/** @param {Record<string, unknown>} report */
export function ingestReport(report) {
  if (shouldIgnoreReport(report)) return null;
  const effectiveDirective = String(
    report['effective-directive'] ?? report.effectiveDirective ?? report['violated-directive'] ?? 'unknown',
  );
  const blocked = String(report['blocked-uri'] ?? report.blockedURL ?? 'unknown');
  const blockedOrig = blockedOrigin(blocked);
  const docPath = documentPath(String(report['document-uri'] ?? report.documentURL ?? ''));
  const key = `${effectiveDirective}|${blockedOrig}|${docPath}`;
  const now = new Date().toISOString();
  let row = aggregates.get(key);
  if (!row) {
    if (aggregates.size >= MAX_KEYS) return key;
    row = {
      count: 0,
      firstSeen: now,
      lastSeen: now,
      effectiveDirective,
      blockedOrigin: blockedOrig,
      documentPath: docPath,
    };
    aggregates.set(key, row);
  }
  row.count += 1;
  row.lastSeen = now;
  return key;
}

export function summaryJson() {
  return {
    processStartedAt,
    keys: aggregates.size,
    maxKeys: MAX_KEYS,
    violations: [...aggregates.values()].sort((a, b) => b.count - a.count),
  };
}

export function resetAggregatesForTests() {
  aggregates.clear();
}
