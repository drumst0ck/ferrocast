const MAX_PAGE = 10000;

/** Strict `?page=` parser: plain positive integers up to MAX_PAGE, anything else is page 1 (no SQL offset overflow). */
export function parsePageParam(raw: string | null): number {
  if (!raw || !/^\d{1,5}$/.test(raw)) return 1;
  const n = Number(raw);
  return n >= 1 && n <= MAX_PAGE ? n : 1;
}

/** Changelog index URL for a page, keeping the tag filter. */
export function changelogHref(page: number, tag: string | null): string {
  const params: string[] = [];
  if (tag) params.push(`tag=${tag}`);
  if (page > 1) params.push(`page=${page}`);
  return params.length ? `/changelog/?${params.join('&')}` : '/changelog/';
}

/** A docs link must stay on this site: an absolute path, not `//host` or `/\host`. */
export function isSafeDocsPath(p: string | null | undefined): p is string {
  return typeof p === 'string' && /^\/(?![/\\])/.test(p);
}
