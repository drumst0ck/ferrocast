/*
 * Reads the hand-written part of a docs tool page (src/content/docs/docs/files/tools/<slug>.mdx) so the generic
 * /tools/<slug>/ page can show the same steps, options and limits. Nothing is rewritten: the doc stays the source.
 * Understands only what those files use: paragraphs, "## Heading" with bullet lists, and ":::caution[Limits]" asides.
 */
export type DocBlock =
  | { kind: 'p'; text: string }
  | { kind: 'h2'; text: string }
  | { kind: 'ul'; items: string[] }
  | { kind: 'note'; title: string; text: string };

export function docBlocks(body: string): DocBlock[] {
  const lines = body
    .replace(/^import .*$/gm, '')
    .replace(/^<ToolFacts [^>]*\/>\s*$/gm, '')
    .split('\n');
  const out: DocBlock[] = [];
  let para: string[] = [];
  let list: string[] | null = null;
  const flushPara = () => { if (para.length) out.push({ kind: 'p', text: para.join(' ') }); para = []; };
  const flushList = () => { if (list) out.push({ kind: 'ul', items: list }); list = null; };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trimEnd();
    const aside = line.match(/^:::(\w+)\[(.+)\]$/);
    if (aside) {
      flushPara(); flushList();
      const text: string[] = [];
      for (i++; i < lines.length && lines[i].trim() !== ':::'; i++) text.push(lines[i].trim());
      out.push({ kind: 'note', title: aside[2], text: text.join(' ') });
      continue;
    }
    const h = line.match(/^## (.+)$/);
    if (h) { flushPara(); flushList(); out.push({ kind: 'h2', text: h[1] }); continue; }
    const li = line.match(/^- (.+)$/);
    if (li) { flushPara(); (list ??= []).push(li[1]); continue; }
    if (!line.trim()) { flushPara(); flushList(); continue; }
    flushPara();
    para.push(line.trim());
  }
  flushPara(); flushList();
  return out;
}

/** Everything before "## Related": the Related list is replaced by the page's own links. */
export function docContentBlocks(body: string): DocBlock[] {
  const blocks = docBlocks(body);
  const end = blocks.findIndex((b) => b.kind === 'h2' && b.text === 'Related');
  return end < 0 ? blocks : blocks.slice(0, end);
}

/** inline() markup plus the <kbd> tags the docs use. */
export function kbdSafe(html: string): string {
  return html.replace(/&lt;kbd&gt;(.*?)&lt;\/kbd&gt;/g, '<kbd>$1</kbd>');
}
