/*
 * Copy for /tools/organize-pdf/ (English). Source: the Notion brief "Organize PDF pages on Mac offline".
 * Ferrocast facts: crates/core/src/convert.rs (parse_tool_query "pages "), crates/core/src/pdfplan.rs
 * (PlanOp labels, on master but not in release v0.3.0), crates/macos/src/pdf.rs (organize writes a new
 * file, refuses a locked PDF), crates/app/src/draw.rs (keys and drag). Preview facts from Apple Support,
 * checked 28 October 2026. Browser organizers are omitted: their price and upload claims were not opened.
 * The mouse buttons exist on master and are not in v0.3.0, so the page says they are coming in the next version.
 */

export const CHECKED = '2026-10-28';

const MOVE = 'https://support.apple.com/guide/preview/add-delete-or-move-pdf-pages-prvw11793/mac';
const ROTATE = 'https://support.apple.com/guide/preview/crop-or-rotate-a-pdf-prvw11567/mac';
const COMBINE = 'https://support.apple.com/guide/preview/combine-pdfs-prvw43696/mac';

export const page = {
  path: '/tools/organize-pdf/',
  title: 'PDF Organizer for Mac: Reorder, Rotate, Delete Pages Offline',
  description:
    'Reorder, rotate, duplicate and remove PDF pages on your Mac with thumbnails or one typed command. Free, open source, nothing uploaded, saves a copy.',
  crumbs: [
    ['Home', '/'],
    ['Tools', '/tools/'],
    ['Organize PDF pages', '/tools/organize-pdf/'],
  ] as [string, string][],
  h1: 'Organize PDF pages on your Mac, offline',
  lead: "Ferrocast's PDF organizer shows a PDF's pages as thumbnails. Reorder them by dragging, rotate, duplicate or remove a page with one key, then press ↵ to save a new copy next to the original. It runs on your Mac and uploads nothing.",
  can: {
    h: 'What you can do',
    intro: 'The current release is keyboard and drag. One page is selected at a time; the keys below act on that page.',
    cols: ['Key', 'Action'],
    rows: [
      ['Drag a thumbnail', 'Reorder pages'],
      ['`⌘←` / `⌘→`', 'Move the selected page earlier or later'],
      ['`R`', 'Rotate the selected page 90° clockwise'],
      ['`⇧R`', 'Rotate it the other way'],
      ['`D`', 'Duplicate the selected page, including its rotation'],
      ['`⌫`', 'Remove the selected page from the copy'],
      ['`↵`', 'Save a new copy next to the original'],
    ],
    buttons:
      'Buttons for each action (◀ Move, Move ▶, ↺ Rotate, ↻ Rotate, Duplicate, Remove) are coming in the next version. Today: keyboard and drag.',
  },
  steps: {
    h: 'How to organize a PDF on Mac',
    howToName: 'Organize PDF pages on a Mac with Ferrocast, without uploading the file',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew. Organizing a PDF does not need Ghostscript or any other optional tool.'],
      ['Open the PDF', 'Drag the PDF from Finder, or select it in the launcher.'],
      ['Open Organize PDF pages', 'Press ⇧⌘O on the PDF result. Or tap ⌘ while dragging to open the Tools wheel and choose Organize PDF pages. The same window is in the Convert view’s editors.'],
      ['Edit the pages', 'Drag thumbnails, or select a page and use the keys in the table above.'],
      ['Press ↵', 'Ferrocast writes a new file next to the original, named with “organized” before .pdf, and never overwrites the original. If nothing changed, it does not write a file.'],
    ] as [string, string][],
  },
  command: {
    h: 'Reorder pages with one typed command',
    paras: [
      'In the launcher, `pages 3,1,2r` puts page 3 first, then page 1, then page 2 rotated 90° clockwise. Page numbers are 1-based. Each `r` is one quarter-turn clockwise; each `l` is one quarter-turn the other way. `pages 3,1,2r,1ll` is the example covered by the parser test: page 3, page 1, page 2 turned once clockwise, then page 1 turned twice (180°).',
      'The new PDF contains only the pages you list, in that order, so a number you leave out is not in the copy. Ranges such as `3-5` are not accepted: list each page. A page number below 1, or a token that is not a number plus `r` and `l`, is rejected and the command does not run.',
    ],
  },
  limits: {
    h: 'Limits',
    items: [
      'A long PDF cannot be dragged past the visible strip. Use `⌘←` and `⌘→` to move the selected page.',
      'It does not merge PDFs. Joining files is a separate tool, [Merge PDFs](/tools/merge-pdf/), with a Choose Order window (`⌘↑` / `⌘↓` or drag).',
      'It does not split a PDF into one file per page. That is [Split PDF](/docs/files/tools/split-pdf/).',
      'It does not compress a PDF. Compress needs Ghostscript. The organizer does not.',
      'It does not edit the text of a PDF. For that, use another app.',
      'A password-protected PDF is refused (`PDF is locked`). There is no unlock step.',
      'Speed on PDFs with hundreds of pages is not measured here, so this page does not promise it.',
    ],
  },
  compare: {
    h: 'Preview vs Ferrocast',
    caption: 'Preview and Ferrocast for organizing one PDF, from Apple Support and the Ferrocast source, 28 October 2026',
    cols: ['', 'Ferrocast', 'Preview (macOS)'],
    rows: [
      ['Price', 'Free (MIT)', 'Included with macOS'],
      ['Upload', 'No', 'No'],
      ['Reorder / delete', 'Yes (drag, `⌘←` / `⌘→`, `⌫`)', 'Yes, from the thumbnail sidebar or Contact Sheet'],
      ['Rotate one page', 'Yes (`R` / `⇧R`)', 'Yes: click to rotate left; hold Option and click to rotate right'],
      ['Duplicate a page', 'Yes (`D`)', 'Not described on the Apple Support pages checked'],
      ['Typed page order', 'Yes (`pages 3,1,2r`)', 'Not described on those pages'],
      ['Output', 'New copy next to the original; the original is not overwritten', 'Not stated on the rearrange page'],
      ['Merge', 'Separate tool ([Merge PDFs](/tools/merge-pdf/))', 'Yes (Combine PDFs). Apple says changes are saved automatically, and to duplicate each PDF first if you want to keep the originals'],
    ],
    note: `Preview from [Add, delete, or move PDF pages](${MOVE}), [Crop or rotate a PDF](${ROTATE}) and [Combine PDFs](${COMBINE}), checked 28 October 2026. Browser organizers are not in this table: their prices and “nothing uploaded” claims were not checked one by one. Ferrocast is independent and not affiliated with Apple or Adobe.`,
  },
  related: {
    h: 'Related PDF tools',
    links: [
      ['Reference: keys and limits', '/docs/files/tools/organize-pdf/'],
      ['Merge PDFs', '/tools/merge-pdf/'],
      ['Split PDF', '/docs/files/tools/split-pdf/'],
      ['PDF conversions', '/convert/pdf/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
      ['All conversions', '/files/'],
      ['All file tools', '/tools/'],
    ] as [string, string][],
  },
  faq: {
    h: 'Questions',
    items: [
      [
        'How do I reorder PDF pages on a Mac without uploading the file?',
        'Open the PDF in Ferrocast’s PDF organizer (⇧⌘O), drag the thumbnails into the new order or move a page with ⌘← and ⌘→, and press ↵. Ferrocast saves a new copy next to the original and never overwrites it. Nothing is uploaded. Preview, built into macOS, can also move and delete pages from its thumbnail sidebar.',
      ],
      [
        'Can I rotate just one page, delete pages, or duplicate a page?',
        'Yes. Select the page and press R to rotate it clockwise, ⇧R the other way, ⌫ to remove it, or D to duplicate it. Preview can rotate one page or several selected pages ([Apple’s crop or rotate page](https://support.apple.com/guide/preview/crop-or-rotate-a-pdf-prvw11567/mac)). Duplicating a page is not described on the Apple Support pages checked.',
      ],
      [
        'Does it overwrite my PDF?',
        'No. ↵ writes a new file next to the original and refuses to replace a file that is already there. The original stays where it was.',
      ],
      [
        'Can I do it from the keyboard or with a typed page order?',
        'Yes. The keys are in the table above. Typed, `pages 3,1,2r` puts page 3 first, then page 1, then page 2 rotated.',
      ],
      [
        'Can Preview do this already?',
        'Yes. [Apple’s guide](https://support.apple.com/guide/preview/add-delete-or-move-pdf-pages-prvw11793/mac) says you can add, delete and rearrange pages from thumbnails, and move pages from one PDF to another. Rotating a page is on a [separate page](https://support.apple.com/guide/preview/crop-or-rotate-a-pdf-prvw11567/mac). Ferrocast is the faster path when you want the keyboard, a typed order, and a copy instead of editing in place.',
      ],
      [
        'Can Ferrocast’s PDF organizer merge PDFs?',
        'No. The organizer works on one PDF’s pages: reorder, rotate, duplicate and remove. Merging is a separate Ferrocast tool, Merge PDFs, which joins two or more PDFs in the order you choose.',
      ],
      [
        'Does it need Ghostscript or anything else?',
        'No. Ghostscript is only used to compress a PDF. The organizer uses PDFKit on your Mac.',
      ],
    ] as [string, string][],
  },
};
