import { createHash, timingSafeEqual } from 'node:crypto';

/** SHA-256 compare so mismatched token lengths do not throw. */
function tokenMatches(presented, expected) {
  const a = createHash('sha256').update(presented).digest();
  const b = createHash('sha256').update(expected).digest();
  return timingSafeEqual(a, b);
}

/**
 * GET /api/csp-report/summary/ is public when CSP_REPORT_SUMMARY_TOKEN is unset or empty.
 * When the variable is set, the request must present it via Authorization: Bearer or ?token=.
 */
export function summaryAccessGranted(authorization, queryToken) {
  const expected = (process.env.CSP_REPORT_SUMMARY_TOKEN ?? '').trim();
  if (!expected) return true;
  const bearer =
    typeof authorization === 'string' && authorization.startsWith('Bearer ')
      ? authorization.slice('Bearer '.length)
      : '';
  const presented = bearer || queryToken || '';
  if (!presented) return false;
  return tokenMatches(presented, expected);
}
