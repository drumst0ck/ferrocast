interface MetaEntry { category: string; title: string | null; bodyMd: string }
interface MetaRelease { version: string; releasedOn: string | null; summary: string | null; entries: MetaEntry[] }

const FILLERS = [' Ferrocast is a fast, open-source macOS launcher.', ' Update with: brew upgrade ferrocast.'];

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** First candidate that fits a search snippet (<= 155); pads a short one up to >= 120. */
function fit(candidates: string[]): string {
  let text = candidates.find((c) => c.length <= 155) ?? candidates[candidates.length - 1].slice(0, 152).replace(/\s+\S*$/, '') + '…';
  for (const f of FILLERS) if (text.length < 120 && (text + f).length <= 155) text += f;
  return text;
}

export function releaseMeta(r: MetaRelease): { title: string; description: string } {
  const unreleased = r.version === 'unreleased';
  const title = unreleased ? 'Ferrocast unreleased changes: next release notes' : `Ferrocast ${r.version} release notes and changes`;
  const total = r.entries.length;
  const count = (c: string) => r.entries.filter((e) => e.category === c).length;
  const parts = [['added', 'added'], ['changed', 'changed'], ['fixed', 'fixed'], ['removed', 'removed'], ['security', 'security']]
    .map(([c, label]) => [count(c), label] as const)
    .filter(([n]) => n > 0)
    .map(([n, label]) => `${n} ${label}`);
  const lead = unreleased ? 'Unreleased changes in Ferrocast' : `Ferrocast ${r.version} (${r.releasedOn ?? 'undated'})`;
  const summary = `${plural(total, 'change', 'changes')}${parts.length ? ': ' + parts.join(', ') : ''}.`;
  const highlights = r.entries.filter((e) => e.title).slice(0, 3).map((e) => e.title as string);
  const withHighlights = highlights.length ? ` Highlights: ${highlights.join(', ')}.` : '';
  const description = fit([
    `${lead}: ${summary}${withHighlights}`,
    `${lead}: ${summary}${highlights.length ? ` Highlights: ${highlights.slice(0, 2).join(', ')}.` : ''}`,
    `${lead}: ${summary}`,
  ]);
  return { title: title.slice(0, 60), description };
}

export function changelogIndexMeta(): { title: string; description: string } {
  return {
    title: 'Ferrocast changelog: every release and fix',
    description: 'What changed in Ferrocast, release by release: new file tools, fixes and known limitations, with an RSS and JSON feed you can subscribe to.',
  };
}
