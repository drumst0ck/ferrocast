/*
 * Copy for /free-file-converter-mac/ (English, board idea "Free local file converter for Mac: Permute /
 * CloudConvert alternative (275 pairs, nothing uploaded)"). Permute facts from
 * software.charliemonroe.net/permute and CloudConvert facts from cloudconvert.com/pricing, read on
 * CHECKED. Ferrocast facts from data/status.ts counts and the conversion docs. Rules: no speed or
 * quality comparison, and say where Permute does more.
 */
import type { GuideCopy } from './compress-video';
import { counts } from '../../data/status';

export const CHECKED = '2026-10-27';

const c = counts();

export const page: GuideCopy = {
  lang: 'en',
  path: '/free-file-converter-mac/',
  title: 'Free file converter for Mac, nothing uploaded | Ferrocast',
  description: `Convert images, video, audio, PDF and documents on your Mac without uploading them: ${c.pairs} conversions and ${c.tools} file tools in a free, open-source launcher.`,
  crumbs: [['Home', '/'], ['Files', '/files/'], ['Free file converter for Mac', '/free-file-converter-mac/']],
  h1: 'A free file converter for Mac that never uploads your files',
  lead: [
    `Ferrocast converts files on your own Mac: drag a file, tap ⇧ and drop it on a format. It covers ${c.pairs} conversions across ${c.inputs} input formats and ${c.tools} file tools such as compress, trim and crop, and writes each result next to the original. It is free and open source (MIT), and it is also a keyboard launcher.`,
    'Permute is a paid Mac converter with more media features; CloudConvert is an online service with a free daily allowance. The table shows what each one’s website says, so you can pick.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Ferrocast, Permute and CloudConvert',
    caption: 'Ferrocast, Permute 4 and CloudConvert compared with facts from their websites',
    cols: ['', 'Ferrocast', 'Permute 4', 'CloudConvert'],
    rows: [
      ['Where it runs', 'On your Mac', 'On your Mac', 'Online service'],
      ['Price', 'Free, no paid plan', '$14.99 one-time (base price in EUR, plus tax), or with Setapp', 'Free plan with 10 conversions a day; paid credit packages and subscriptions'],
      ['Formats', `${c.pairs} conversions across ${c.inputs} input formats: images, video, audio, PDF, documents, slides, spreadsheets, subtitles and archives`, '“100+ formats” across video, audio, images, PDF and text', 'Not compared here'],
      ['Extra tools', `${c.tools} file tools: compress, trim, crop, redact, merge PDF and more`, 'Text to audio narration, OCR, PDF pages to images, transcription to text or SRT', 'Not compared here'],
      ['Limits', 'Some pairs need optional ffmpeg, LibreOffice, cwebp, Ghostscript or unrar', 'The App Store version omits some capabilities, per its website', 'Free plan: 1 GB per file, 5 minutes of processing'],
      ['Requirements', 'Apple Silicon, macOS 13 or later', 'Permute 4: macOS 26 and Apple Silicon; Permute 3 for macOS 11 to 15 and Intel', 'A browser'],
      ['Source code', 'MIT', 'No open licence stated', 'No open licence stated'],
    ],
    note: 'Permute from [its website](https://software.charliemonroe.net/permute/) and CloudConvert from [its pricing page](https://cloudconvert.com/pricing), checked 27 October 2026. Ferrocast from the [conversion reference](/docs/files/conversions/).',
  },
  steps: {
    h: 'Convert a file',
    howToName: 'Convert a file on a Mac with Ferrocast, without uploading it',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew.'],
      ['Drag the file', 'Start dragging one or more files from Finder, or select them in the launcher.'],
      ['Tap ⇧', 'The conversion wheel opens with the formats that file can become.'],
      ['Drop on a format', 'The result is written next to the original and never overwrites it.'],
    ],
  },
  sections: [
    {
      h: 'Most searched conversions',
      list: [
        '[HEIC to JPG](/convert/heic-to-jpg/) · [WebP to JPG](/convert/webp-to-jpg/) · [PNG to JPG](/convert/png-to-jpg/)',
        '[MOV to MP4](/convert/mov-to-mp4/) · [MOV to GIF](/convert/mov-to-gif/) · [compress a video to a size](/compress-video-to-size/)',
        '[PDF to DOCX](/convert/pdf-to-docx/) · [DOCX to PDF](/convert/docx-to-pdf/)',
        '[Every conversion, by family](/files/) · [all file tools](/tools/)',
      ],
    },
    {
      h: 'Where Permute does more',
      paras: ['Permute’s website lists text-to-speech narration, OCR and transcription to SRT, and it runs on Intel Macs with Permute 3. If you need those, it is the better pick. Ferrocast’s strength is that conversion lives in the launcher you already use, for free.'],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is there a free file converter for Mac that does not upload files?', `Yes. Ferrocast converts files on your Mac (${c.pairs} conversions across ${c.inputs} input formats) and writes the result next to the original. Nothing is uploaded.`],
      ['Do I need ffmpeg?', 'Not for many conversions, because Ferrocast tries macOS tools first. Some pairs need ffmpeg, LibreOffice, cwebp, Ghostscript or unrar; they say so and Ferrocast shows the install command.'],
      ['Can I convert several files at once?', 'Yes. Drag or select several files and drop them on a format; each one is converted next to its original.'],
      ['How much does Permute cost?', 'Permute 4 is $14.99 as a one-time licence according to its website (base price in EUR, tax added), or part of a Setapp subscription. It needs macOS 26 and Apple Silicon.'],
      ['Is CloudConvert free?', 'CloudConvert’s free plan allows 10 conversions a day, files up to 1 GB, according to its pricing page.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Conversion reference', '/docs/files/conversions/'],
      ['Optional tools', '/docs/files/optional-tools/'],
      ['Convertir archivos (español)', '/es/convertir/'],
      ['Key facts', '/facts/'],
    ],
  },
};
