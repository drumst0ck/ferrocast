import conversions from '../../data/conversions.json';

export interface Pair {
  slug: string;
  family: string;
  familyName: string;
  inExt: string;
  inLabel: string;
  outExt: string;
  outLabel: string;
  needs: string | null;
}

const INSTALL: Record<string, string> = {
  ffmpeg: 'brew install ffmpeg',
  LibreOffice: 'brew install --cask libreoffice',
  cwebp: 'brew install webp',
  unrar: 'brew install --cask rar',
  Ghostscript: 'brew install ghostscript',
};

export function installCommand(tool: string): string {
  const cmd = INSTALL[tool];
  if (!cmd) throw new Error(`no install command known for optional tool "${tool}"`);
  return cmd;
}

const slugPart = (s: string) => s.toLowerCase();

// conversions.json labels some formats just "Other"; show their real names so pages stay distinct.
const NAMES: Record<string, string> = {
  rtf: 'RTF', rtfd: 'RTFD', textClipping: 'Text clipping', svg: 'SVG', ppt: 'PPT', xls: 'XLS',
};
const display = (ext: string, label: string) => (label === 'Other' ? (NAMES[ext] ?? ext.toUpperCase()) : label);

export function allPairs(): Pair[] {
  return conversions.families.flatMap((f) =>
    f.inputs.flatMap((i) =>
      i.outputs.map((o) => ({
        slug: `${slugPart(i.ext)}-to-${slugPart(o.ext)}`,
        family: f.id,
        familyName: f.name,
        inExt: i.ext,
        inLabel: display(i.ext, i.label),
        outExt: o.ext,
        outLabel: display(o.ext, o.label),
        needs: (o as { needs: string | null }).needs,
      })),
    ),
  );
}

export function familyHubs() {
  return conversions.families.map((f) => ({
    id: f.id,
    name: f.name,
    inputs: f.inputs.map((i) => ({ ext: i.ext, label: display(i.ext, i.label), outputs: i.outputs.map((o) => ({ ext: o.ext, label: display(o.ext, o.label) })) })),
  }));
}

export function pairMeta(p: Pair): { title: string; description: string } {
  const titles = [
    `Convert ${p.inLabel} to ${p.outLabel} on Mac, locally · Ferrocast`,
    `Convert ${p.inLabel} to ${p.outLabel} on Mac · Ferrocast`,
    `${p.inLabel} to ${p.outLabel} on Mac · Ferrocast`,
  ];
  const title = titles.find((t) => t.length <= 60) ?? titles[titles.length - 1];
  const lead = `Convert ${p.inLabel} to ${p.outLabel} on your Mac, locally.`;
  // Most complete sentence that still fits a search snippet (<=155 characters).
  const candidates = p.needs
    ? [
        `${lead} Needs ${p.needs} (optional, free); Ferrocast shows the install command. Drag the file, tap ⇧, pick ${p.outLabel}.`,
        `${lead} Needs ${p.needs} (optional, free); Ferrocast shows the install command. Drag the file, tap ⇧.`,
        `${lead} Needs ${p.needs} (optional, free); Ferrocast shows the install command. Free and open source.`,
        `${lead} Needs ${p.needs} (optional); Ferrocast shows the install command. Drag a file, tap ⇧.`,
      ]
    : [
        `${lead} No extra tool needed: macOS does it. Drag the file, tap ⇧, pick ${p.outLabel}. Free and open source.`,
        `${lead} No extra tool needed: macOS does it. Drag the file, tap ⇧, pick ${p.outLabel}.`,
        `${lead} No extra tool needed: macOS does it. Drag the file, tap ⇧. Free and open source.`,
        `${lead} No extra tool needed: macOS does it. Drag the file, tap ⇧. Open source.`,
      ];
  const description = candidates.find((c) => c.length <= 155) ?? candidates[candidates.length - 1];
  return { title, description };
}

export function pairFacts(p: Pair): { requirement: string; limits: string[]; status: string } {
  const requirement = p.needs
    ? `This pair needs ${p.needs}, which is optional and not bundled with Ferrocast. If it is missing, Ferrocast shows the install command: ${installCommand(p.needs)}.`
    : 'No extra tool needed: this pair runs with the tools that ship with macOS.';
  const limits: string[] = [];
  if (['pdf', 'documents', 'slides', 'spreadsheets'].includes(p.family) || ['pdf', 'epub'].includes(p.outExt)) {
    limits.push('Layout fidelity is not checked: results are verified by reading them back (text inside documents and sheets), not by comparing how they look.');
  }
  if (p.outExt === 'pages') limits.push(".pages is written in the iWork '09 format and was opened with LibreOffice, not with Pages.");
  if (p.inExt === 'rar') limits.push('RAR extraction was only tested with a hand-built archive, because the unrar available during testing is x86-only.');
  if (p.inExt === 'pdf' && ['png', 'jpg'].includes(p.outExt)) limits.push('Every page is exported, at 300 DPI.');
  return {
    requirement,
    limits,
    status: 'Tested: every one of the 275 pairs has been run on real files in tests. The drag wheel gesture itself has not been tried in a live session (Built).',
  };
}

export function relatedPairs(p: Pair) {
  const all = allPairs();
  return {
    reverse: all.find((q) => q.inExt === p.outExt && q.outExt === p.inExt) ?? null,
    sameInput: all.filter((q) => q.inExt === p.inExt && q.outExt !== p.outExt).slice(0, 5),
    sameOutput: all.filter((q) => q.outExt === p.outExt && q.inExt !== p.inExt).slice(0, 4),
  };
}
