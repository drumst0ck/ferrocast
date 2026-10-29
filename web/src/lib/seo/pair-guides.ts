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
  /** Replaces the page <title> (see pairMeta). */
  title?: string;
  /** Replaces the meta description (120-155 characters). */
  description?: string;
  /** Replaces the template <h1>. */
  h1?: string;
  /** Replaces the template lead sentence. */
  lead: string;
  /** The question the lead answers; shown as the first FAQ entry (and in the FAQPage JSON-LD) with `lead` as its answer. */
  question?: string;
  sections: GuideSection[];
  /** Extra FAQ entries, appended to the template ones. */
  faq: [string, string][];
}

const FINDER = 'https://support.apple.com/guide/mac-help/perform-quick-actions-in-the-finder-on-mac-mchl97ff9142/mac';
const PREVIEW = 'https://support.apple.com/guide/preview/convert-image-file-types-prvw1012/mac';

const TEXTEDIT = 'https://support.apple.com/guide/textedit/txtee6663a0e/mac';
const TEXTUTIL = 'https://keith.github.io/xcode-man-pages/textutil.1.html';

export const pairGuides: Record<string, PairGuide> = {
  // Search Console 22-27 Oct 2026: near the top 10 with no clicks. Native routes are only quoted where Apple's
  // TextEdit guide or the textutil man page say so; the other pairs name Ferrocast alone.
  'rtf-to-txt': {
    title: 'How to Convert RTF to TXT on Mac (Free, No Upload) | Ferrocast',
    description: 'Turn an RTF into a plain .txt file on your Mac: drag it into Ferrocast and pick Plain text, or use TextEdit. Free, open source, nothing uploaded.',
    h1: 'How to convert RTF to TXT on a Mac',
    question: 'How do I convert an RTF file to TXT on a Mac?',
    lead: `To convert an RTF file to TXT on a Mac, drag it onto Ferrocast, tap ⇧ and choose Plain text; the .txt is saved next to the original and nothing is uploaded. Without extra apps, open it in TextEdit and choose Format → Make Plain Text, then save ([Apple](${TEXTEDIT})).`,
    sections: [
      {
        h: 'Convert RTF to TXT in Terminal',
        paras: [`Ferrocast runs macOS's own \`textutil\` for this pair ([man page](${TEXTUTIL})). To do the same yourself:`],
        code: 'textutil -convert txt file.rtf',
      },
    ],
    faq: [],
  },
  'md-to-pages': {
    title: 'Convert Markdown (.md) to Pages on Mac, Free | Ferrocast',
    description: 'Turn a Markdown file into a Pages document on your Mac without uploading it. Drag the .md into Ferrocast and pick Pages. Free and open source.',
    h1: 'Convert Markdown to Pages on a Mac',
    question: 'How do I convert a Markdown file to Pages on a Mac?',
    lead: 'To convert a Markdown file to Pages on a Mac, drag the .md onto Ferrocast, tap ⇧ and release over Pages; the .pages document is saved next to the original and nothing is uploaded. It needs no extra tool.',
    sections: [],
    faq: [],
  },
  'rtf-to-html': {
    title: 'Convert RTF to HTML on Mac for Free, Offline | Ferrocast',
    description: 'Turn an RTF document into an HTML file on your Mac. Drag it into Ferrocast and pick HTML, or use textutil. Free, open source, nothing uploaded.',
    h1: 'How to convert RTF to HTML on a Mac',
    question: 'How do I convert an RTF file to HTML on a Mac?',
    lead: `To convert an RTF file to HTML on a Mac, drag it onto Ferrocast, tap ⇧ and choose HTML; the .html is saved next to the original and nothing is uploaded. Without extra apps, run \`textutil -convert html file.rtf\` in Terminal ([man page](${TEXTUTIL})).`,
    sections: [],
    faq: [],
  },
  'flac-to-wav': {
    title: 'Convert FLAC to WAV on Mac for Free, Offline | Ferrocast',
    description: 'Turn FLAC audio into WAV on your Mac without uploading it. Drag the files into Ferrocast and pick WAV; batches work too. Free and open source.',
    h1: 'Convert FLAC to WAV on a Mac',
    question: 'How do I convert FLAC to WAV on a Mac?',
    lead: 'To convert FLAC to WAV on a Mac, drag the files onto Ferrocast, tap ⇧ and release over WAV; each .wav is saved next to its original and nothing is uploaded. It needs no extra tool.',
    sections: [],
    faq: [],
  },
  'flac-to-mp3': {
    title: 'Convert FLAC to MP3 on Mac for Free, Offline | Ferrocast',
    description: 'Turn FLAC audio into MP3 on your Mac without uploading it. Drag the files into Ferrocast and pick MP3. Needs the free ffmpeg. Free and open source.',
    h1: 'Convert FLAC to MP3 on a Mac',
    question: 'How do I convert FLAC to MP3 on a Mac?',
    lead: 'To convert FLAC to MP3 on a Mac, drag the files onto Ferrocast, tap ⇧ and release over MP3; each .mp3 is saved next to its original and nothing is uploaded. This pair needs the free ffmpeg (`brew install ffmpeg`); Ferrocast shows the command if it is missing.',
    sections: [],
    faq: [],
  },
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
          '**Record straight to GIF:** record with Ferrocast, edit with auto zoom, then export as GIF at 480, 720 or 960 px wide and 10 or 15 fps. See [screen capture](/docs/launcher/screen-capture/) and [the recorder page](/screen-studio-alternative/).',
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
