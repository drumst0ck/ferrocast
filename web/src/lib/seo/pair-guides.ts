/*
 * Extra guide sections for the most searched English pair pages (/convert/<slug>/), from the SEO board
 * ideas "HEIC to JPG as a guide", "Turn a Mac screen recording into a GIF" and "PDF to Word on Mac
 * without uploading". Facts: Apple Support (Finder quick actions, Preview export), crates/core/src/
 * convert.rs (engines and ffmpeg arguments) and the screen-capture docs (GIF export sizes and limits).
 * Never promise kept metadata or quality that was not checked.
 */

export interface GuideSection {
  h: string;
  paras?: string[];
  list?: string[];
  code?: string;
}

export interface PairGuide {
  /** Replaces the template lead sentence. */
  lead: string;
  sections: GuideSection[];
  /** Extra FAQ entries, appended to the template ones. */
  faq: [string, string][];
}

const FINDER = 'https://support.apple.com/guide/mac-help/perform-quick-actions-in-the-finder-on-mac-mchl97ff9142/mac';
const PREVIEW = 'https://support.apple.com/guide/preview/convert-image-file-types-prvw1012/mac';

export const pairGuides: Record<string, PairGuide> = {
  'heic-to-jpg': {
    lead: 'iPhone photos arrive on the Mac as HEIC, and some apps and websites still want JPG. There are four ways to convert them on a Mac without uploading anything; Ferrocast is the one that also does it in bulk from a drag.',
    sections: [
      {
        h: 'Four ways to convert HEIC to JPG on a Mac',
        list: [
          `**Finder quick action:** Control-click the photo, choose Quick Actions → Convert Image, then pick JPEG and a size ([Apple](${FINDER})).`,
          `**Preview:** open the photo, choose File → Export and pick JPEG; you can set the quality. To do several at once, open them in one window, select them in the sidebar and export ([Apple](${PREVIEW})).`,
          '**Terminal:** macOS includes `sips`, the same tool Ferrocast uses for this pair (see below).',
          '**Ferrocast:** drag one or many photos, tap ⇧ and drop on JPEG. Each JPG is written next to its original.',
        ],
      },
      {
        h: 'The same conversion in Terminal',
        paras: ['Ferrocast runs `sips -s format jpeg` for this pair. To convert every HEIC in a folder yourself:'],
        code: 'for f in *.heic; do sips -s format jpeg "$f" --out "${f%.heic}.jpg"; done',
      },
      {
        h: 'Before you share the JPG',
        paras: ['Photos can carry the place where they were taken. If that matters, check the file’s information in Finder or Preview before you send it. Ferrocast’s [Edit metadata](/tools/edit-metadata/) tool can remove metadata from JPEG files.'],
      },
    ],
    faq: [
      ['Can I convert many HEIC photos at once?', 'Yes. Select or drag several photos and drop them on JPEG; each one is converted next to its original.'],
      ['Which tool does Ferrocast use for HEIC to JPG?', 'sips, the image tool that ships with macOS, so nothing else needs to be installed.'],
    ],
  },
  'mov-to-gif': {
    lead: 'Screen recordings on a Mac are .mov files. Ferrocast turns one into a GIF from a drag, and its own screen recorder can export a GIF directly, with auto zoom on your clicks.',
    sections: [
      {
        h: 'Two ways to get a GIF of your screen',
        list: [
          '**Convert an existing recording:** drag the .mov (from Screenshot.app or QuickTime), tap ⇧ and drop on GIF. Ferrocast runs ffmpeg at 12 frames per second and 640 px wide.',
          '**Record straight to GIF:** record with Ferrocast, edit with auto zoom, then export as GIF at 480, 720 or 960 px wide and 10 or 15 fps. See [screen capture](/docs/launcher/screen-capture/).',
        ],
      },
      {
        h: 'Keep the GIF short',
        paras: ['A GIF stores every frame, so files grow fast. Trim the clip first with [Trim video](/tools/trim-video/). The recorder’s GIF export is limited in length because it keeps all frames in memory: about 20 seconds at 960 px and 15 fps, about 80 seconds at 480 px.'],
      },
      {
        h: 'What the conversion uses',
        paras: ['MOV to GIF needs ffmpeg (`brew install ffmpeg`). The arguments are `-vf fps=12,scale=640:-1:flags=lanczos`, so the height follows the video’s proportions.'],
      },
    ],
    faq: [
      ['Can I choose the GIF size when converting a MOV?', 'Not in the conversion: it is 12 fps and 640 px wide. The screen recorder’s GIF export lets you choose 480, 720 or 960 px and 10 or 15 fps.'],
      ['Does the GIF have sound?', 'No. GIF has no audio track.'],
    ],
  },
  'pdf-to-docx': {
    lead: 'Online PDF to Word converters upload your document. Ferrocast converts the PDF on your Mac with LibreOffice, a free office suite, and writes the .docx next to the original.',
    sections: [
      {
        h: 'What to expect from the result',
        list: [
          'Simple documents (text, headings, a few images) convert best.',
          'Multi-column layouts, complex tables and heavy design may need fixing in Word or Pages.',
          'Check the document before you send it.',
        ],
      },
      {
        h: 'Set it up once',
        paras: ['PDF to DOCX needs LibreOffice. If it is missing, Ferrocast shows the install command:'],
        code: 'brew install --cask libreoffice',
      },
    ],
    faq: [
      ['Do I need Microsoft Word to convert a PDF?', 'No. Ferrocast uses LibreOffice to convert. You can open the .docx in Word, Pages or LibreOffice.'],
      ['Is my PDF uploaded?', 'No. LibreOffice converts it on your Mac.'],
    ],
  },
};
