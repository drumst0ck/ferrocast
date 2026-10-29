/*
 * Own content for ten /tools/<slug>/ pages that Google crawled and left out of the index (Search Console, 29 Oct 2026),
 * from the SEO board brief "Páginas /tools/: contenido propio frente a /docs/files/tools/". Every option and limit comes
 * from the tool's docs page (src/content/docs/docs/files/tools/<slug>.mdx), from docs/files/typed-commands.md and
 * optional-tools.md, or from the source where the docs say nothing about the output name (crates/core/src/convert.rs:
 * FileTool::suffix and output_with_suffix, crates/app/src/services.rs: SplitPages). Native macOS alternatives are
 * quoted only where Apple's own guides say so (checked 29 Oct 2026); the others are left out.
 * Shortcut and input formats are not written here: the page reads them from tools.json (ToolPage).
 */

export interface ToolSection { h: string; paras?: string[]; list?: string[] }

export interface ToolGuide {
  slug: string;
  title: string;
  description: string;
  h1: string;
  lead: string;
  howToName: string;
  /** The H2 above the numbered steps: "How to <task> on your Mac". */
  stepsH: string;
  steps: [string, string][];
  sections: ToolSection[];
  faq: [string, string][];
  /** Chip links under the FAQ; the docs reference link is added by the page. */
  related: [string, string][];
}

const FINDER = 'https://support.apple.com/guide/mac-help/perform-quick-actions-in-the-finder-on-mac-mchl97ff9142/mac';
const PREVIEW_PAGES = 'https://support.apple.com/guide/preview/add-delete-or-move-pdf-pages-prvw11793/mac';
const PREVIEW = 'https://support.apple.com/guide/preview/';
const PHOTOS = 'https://support.apple.com/guide/photos/';

const INSTALL = 'Download the DMG or install it with Homebrew.';
const NEEDS_FFMPEG = 'It needs the free ffmpeg (`brew install ffmpeg`); if it is missing, Ferrocast shows the install command instead of failing.';
const FFMPEG_LIST = 'ffmpeg is one of five optional helpers. Ferrocast shows its install command, `brew install ffmpeg`, when it is missing.';

export const toolGuides: Record<string, ToolGuide> = {
  'split-pdf': {
    slug: 'split-pdf',
    title: 'Split a PDF on Mac for Free, Offline | Ferrocast',
    description: 'Split a PDF into one file per page on your Mac, with nothing uploaded. Choose Split into pages in the Convert view; each page is saved as a new PDF.',
    h1: 'How to split a PDF on a Mac without uploading it',
    lead: 'Ferrocast splits a PDF into one file per page on your Mac. Open the PDF in the Convert view, choose Split into pages, and every page is saved as its own PDF next to the original. Nothing is uploaded, the original is never overwritten, and the tool is free and open source (MIT).',
    howToName: 'Split a PDF on a Mac with Ferrocast, without uploading it',
    stepsH: 'How to split a PDF on your Mac',
    steps: [
      ['Install Ferrocast', `${INSTALL} Splitting a PDF needs no optional helper such as ffmpeg or Ghostscript.`],
      ['Select the PDF', 'Drag it from Finder, or select it in the launcher so that it shows as a file result.'],
      ['Open the Convert view', 'Press ⌥↵ on the PDF result, or press ⌘T. You can also tap ⌘ while dragging the file to open the Tools wheel.'],
      ['Choose Split into pages', 'It is a one-step preset, so there is no window to fill in: choosing it starts the split.'],
      ['Open the pages', 'Each page is written as its own PDF in the same folder, named after the original with “page” and the page number, for example `report page 1.pdf`. An existing file is never overwritten.'],
    ],
    sections: [
      {
        h: 'What this tool does and does not do',
        list: [
          'It takes PDF files only and writes one PDF per page, in page order.',
          'It has no options to set and no shortcut. The Tools wheel and the Convert view both offer it.',
          'It does not choose pages for you. To keep only some pages, or to change their order, use [Organize PDF pages](/tools/organize-pdf/), which saves one new PDF with the pages you list.',
          'To put separate PDFs back into one file, use [Merge PDFs](/tools/merge-pdf/).',
        ],
      },
      {
        h: 'Other ways to split a PDF on a Mac',
        paras: [
          `Preview can do it by hand. Apple’s Preview guide says that you can create a new PDF by dragging page thumbnails from the sidebar to the desktop ([Apple](${PREVIEW_PAGES})). That works well for one or two pages; for a long document Ferrocast writes every page in one step.`,
        ],
      },
      {
        h: 'Before you split a long PDF',
        paras: [
          'A 100-page PDF becomes 100 files in the folder it came from. Put the PDF in its own folder first and the pages end up together instead of among your other files.',
          'The original stays as it was, so you can delete the pages you do not need and keep the rest.',
        ],
      },
      {
        h: 'Reading the file names',
        paras: [
          'The first page is `report page 1.pdf`, the second is `report page 2.pdf`, and so on, if the PDF is called `report.pdf`. If you split the same PDF again, the new files get a number added to the name, for example `report page 1 2.pdf`, instead of replacing the earlier ones.',
        ],
      },
    ],
    faq: [
      ['Does splitting a PDF upload it anywhere?', 'No. The split runs on your Mac and the pages are written to the folder the PDF is in.'],
      ['Does Split PDF need ffmpeg or Ghostscript?', 'No. Ghostscript is only used to compress PDFs and ffmpeg is for audio and video, so splitting a PDF needs neither.'],
      ['Can I choose which pages to extract?', 'Not with this tool, which writes every page. Use Organize PDF pages to save only the pages you list, in the order you list them.'],
      ['Does it work on an Intel Mac?', 'No. Ferrocast runs on Apple Silicon Macs only.'],
    ],
    related: [
      ['Merge PDFs', '/tools/merge-pdf/'],
      ['Organize PDF pages', '/tools/organize-pdf/'],
      ['Make a PDF from images', '/tools/create-pdf/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'create-pdf': {
    slug: 'create-pdf',
    title: 'Make a PDF from Images on Mac for Free | Ferrocast',
    description: 'Combine images into one PDF on your Mac with a page per image, in the order you choose. Free, open source, nothing uploaded.',
    h1: 'How to make a PDF from images on a Mac without uploading them',
    lead: 'Ferrocast combines selected images into one PDF on your Mac, with one page per image. Select the images, choose Combine into one PDF, put them in order in the Choose Order window, and the PDF is saved next to the originals. Nothing is uploaded and no image is overwritten.',
    howToName: 'Make a PDF from images on a Mac with Ferrocast, without uploading them',
    stepsH: 'How to make a PDF from images on your Mac',
    steps: [
      ['Install Ferrocast', INSTALL],
      ['Select the images', 'Select several images in Finder or in the launcher: JPEG, PNG, WebP, HEIC, TIFF or SVG files.'],
      ['Choose Combine into one PDF', 'Tap ⌘ while dragging the images to open the Tools wheel, or open the Convert view, and choose Combine into one PDF.'],
      ['Set the page order', 'The Choose Order window lists your images. Move a row with ⌘↑ and ⌘↓, or drag rows. The PDF has one page per image, in that order.'],
      ['Open the PDF', 'Ferrocast writes the PDF next to the originals. It never overwrites an existing file.'],
    ],
    sections: [
      {
        h: 'What you can set',
        list: [
          'Input: JPEG, PNG, WebP, HEIC, TIFF and SVG images.',
          'Order: set in the Choose Order window, with ⌘↑ and ⌘↓ or by dragging rows.',
          'It is a one-step preset. The docs list no other options, so there is no page size or quality setting to choose.',
        ],
      },
      {
        h: 'Try another order',
        paras: [
          'The PDF is a new file and the images stay as they were. If you want the pages in a different order, run the tool again: Ferrocast never overwrites, so the second PDF gets its own name and the first one is still there.',
        ],
      },
      {
        h: 'Fix the order afterwards',
        paras: [
          'If a page ends up in the wrong place, open the new PDF in [Organize PDF pages](/tools/organize-pdf/). You can drag pages, rotate, duplicate or remove them and save a copy, without making the PDF again from the images.',
          'To join PDFs you already have, use [Merge PDFs](/tools/merge-pdf/); to cut a PDF into single pages, use [Split PDF](/tools/split-pdf/).',
        ],
      },
      {
        h: 'Other ways to make a PDF on a Mac',
        paras: [
          `Finder has a Create PDF quick action: Control-click a file and choose Quick Actions ([Apple’s Mac guide](${FINDER})). Ferrocast adds the Choose Order window, so you decide the page order before the PDF is written.`,
        ],
      },
      {
        h: 'One image, one page',
        paras: [
          'Every selected image becomes exactly one page, so ten images make a ten-page PDF. Only the formats listed in the facts table above are inputs for this tool; select images in those formats and leave other files out of the selection.',
        ],
      },
    ],
    faq: [
      ['Are my images uploaded when I make the PDF?', 'No. Ferrocast builds the PDF on your Mac and writes it to the folder of the originals.'],
      ['Does it change or replace the original images?', 'No. The PDF is a new file next to the images, and Ferrocast never overwrites an existing file.'],
      ['Can I change the page order after the PDF is made?', 'Yes. Open the PDF in Organize PDF pages, move, rotate, duplicate or remove pages and save a new copy.'],
      ['Is making a PDF from images free?', 'Yes. Ferrocast is free and open source (MIT) and runs on Apple Silicon Macs only.'],
    ],
    related: [
      ['Merge PDFs', '/tools/merge-pdf/'],
      ['Split PDF', '/tools/split-pdf/'],
      ['Organize PDF pages', '/tools/organize-pdf/'],
      ['Photo collage', '/tools/collage/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'crop-image': {
    slug: 'crop-image',
    title: 'Crop an Image on Mac for Free, Offline | Ferrocast',
    description: 'Crop an image on your Mac with a draggable rectangle, a 16:9 or 1:1 preset, or exact width and height. Free, nothing uploaded, saves a copy.',
    h1: 'How to crop an image on a Mac without uploading it',
    lead: 'Ferrocast crops an image on your Mac with a draggable rectangle that has eight handles. Choose an aspect preset such as 1:1 or 16:9, or type an exact width and height, then press ↵ to save the cropped copy next to the original. Nothing is uploaded and the original is never overwritten.',
    howToName: 'Crop an image on a Mac with Ferrocast, without uploading it',
    stepsH: 'How to crop an image on your Mac',
    steps: [
      ['Install Ferrocast', INSTALL],
      ['Select the image', 'Drag a JPEG, PNG, WebP, HEIC or TIFF image from Finder, or select it in the launcher.'],
      ['Open the crop editor', 'Press ⇧⌘K on the image result, or tap ⌘ while dragging to open the Tools wheel and choose Crop image.'],
      ['Draw the crop', 'Drag the rectangle by its eight handles, pick an aspect preset, or type the width and height in the boxes.'],
      ['Press ↵', 'The cropped copy appears next to the original. Ferrocast never overwrites a file.'],
    ],
    sections: [
      {
        h: 'What you can set',
        list: [
          'A draggable rectangle with eight handles.',
          'Aspect presets: 1:1, 4:3, 3:2, 16:9 and 9:16.',
          'Exact width and height boxes, for a crop of a known size.',
          'Input: JPEG, PNG, WebP, HEIC and TIFF.',
        ],
      },
      {
        h: 'Crop with a typed command',
        paras: [
          'Typed commands skip the window. In the Convert view for an image, type `800x600` and press ↵ for an exact 800 × 600 crop. The result is written next to the original, like every other Ferrocast result. The other commands are in the [typed commands reference](/docs/files/typed-commands/).',
        ],
      },
      {
        h: 'Other ways to crop an image on a Mac',
        paras: [
          `Preview can crop too. Apple’s Preview guide describes selecting an area of an image with the shape selection tools in the Markup toolbar and then cropping it ([Apple’s Preview guide](${PREVIEW})). Ferrocast adds the aspect presets and exact-size boxes, and it saves a copy beside the original.`,
        ],
      },
      {
        h: 'A common order of work',
        paras: [
          'Crop first, then adjust the picture with [Edit photos](/tools/edit-photos/), then place it on a canvas with [Add a background](/tools/add-background/). Each step writes a new copy next to the file it started from, so the earlier versions stay as they were.',
        ],
      },
      {
        h: 'Cropping video as well',
        paras: [
          'The same idea works for a video: [Crop video](/tools/crop-video/) crops the frame of a clip with a rectangle, presets or exact size, and keeps the audio.',
        ],
      },
      {
        h: 'Choosing an aspect preset',
        paras: [
          'A square 1:1 crop suits a profile picture, 16:9 suits a widescreen banner or a slide, 9:16 suits a phone story, and 4:3 and 3:2 are the shapes of many camera photos. If none of them fits, type the width and height you need.',
        ],
      },
    ],
    faq: [
      ['Is my image uploaded when I crop it?', 'No. The crop happens on your Mac and the copy is written to the original’s folder.'],
      ['Can I crop to a fixed aspect ratio?', 'Yes. Choose 1:1, 4:3, 3:2, 16:9 or 9:16, or use the width and height boxes for an exact size.'],
      ['Does cropping replace my original photo?', 'No. Press ↵ and Ferrocast writes a cropped copy; it never overwrites the original.'],
      ['Will it run on an Intel Mac?', 'No. Ferrocast is for Apple Silicon Macs only.'],
    ],
    related: [
      ['Crop video', '/tools/crop-video/'],
      ['Edit photos', '/tools/edit-photos/'],
      ['Add a background to an image', '/tools/add-background/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'crop-video': {
    slug: 'crop-video',
    title: 'Crop a Video on Mac for Free, Offline | Ferrocast',
    description: 'Crop the frame of a video on your Mac with a draggable rectangle, an aspect preset or exact size. Keeps the audio. Needs the free ffmpeg.',
    h1: 'How to crop a video on a Mac without uploading it',
    lead: 'Ferrocast crops the frame of a video on your Mac. The crop window shows a real frame of the clip with a draggable rectangle, aspect presets and exact width and height boxes, and the audio is kept. The cropped copy is saved next to the original; nothing is uploaded and the video is never overwritten.',
    howToName: 'Crop a video on a Mac with Ferrocast, without uploading it',
    stepsH: 'How to crop a video on your Mac',
    steps: [
      ['Install Ferrocast and ffmpeg', `${INSTALL} ${NEEDS_FFMPEG}`],
      ['Select the video', 'Choose the clip to crop (MP4, MOV or MKV) by dragging it from Finder or selecting it in the launcher.'],
      ['Open the crop window', 'Press ⇧⌘J on the video result, or tap ⌘ while dragging to open the Tools wheel and choose Crop video.'],
      ['Set the crop', 'Drag the rectangle, pick an aspect preset, or type the width and height. The window shows a frame from the video behind it.'],
      ['Press ↵', 'Ferrocast writes the cropped copy next to the original. It never overwrites a file.'],
    ],
    sections: [
      {
        h: 'What you can set',
        list: [
          'A draggable crop rectangle on a real frame of the video.',
          'Aspect presets, and exact width and height boxes.',
          'The audio is kept.',
          'Rotated phone video is handled: for a ±90° or 270° rotation, the displayed width and height swap, so the crop matches what you see.',
          'Input: MP4, MOV and MKV.',
        ],
      },
      {
        h: 'Crop with a typed command',
        paras: [
          'In the Convert view for a video, type `crop 640x360 at 100,50` and press ↵ to crop to 640 × 360 at the position 100,50 without opening the window. The other typed commands are in the [typed commands reference](/docs/files/typed-commands/).',
        ],
      },
      {
        h: 'A common order of work',
        paras: [
          'For a clip that is too long and too wide, cut it first with [Trim video](/tools/trim-video/) or [Split video](/tools/split-video/), then crop it here, and join the pieces afterwards with [Join videos](/tools/join-videos/). Every step writes a new copy next to the file it started from, so you can go back to an earlier version.',
        ],
      },
      {
        h: 'Limits to know about',
        list: [
          'Cropping needs ffmpeg, the free helper that Ferrocast does not bundle. ' + FFMPEG_LIST,
          'The frame shown in the window is fixed at 10% of the video’s duration.',
          'WMV files fail on crop.',
        ],
      },
      {
        h: 'Choosing a crop shape',
        paras: [
          'Use 16:9 for a widescreen video, 9:16 for a vertical clip for a phone, or 1:1 for a square post. If a site or an app asks for a size in pixels, type the width and height in the boxes instead of dragging the rectangle.',
        ],
      },
    ],
    faq: [
      ['Does cropping a video need ffmpeg?', 'Yes. Crop video needs the free ffmpeg. Install it with `brew install ffmpeg`, or let Ferrocast show you the command when it is missing.'],
      ['Does the cropped video keep its sound?', 'Yes. The crop window keeps the audio.'],
      ['Can I crop a video I filmed on a phone?', 'Yes. Rotated phone video is handled: the width and height shown in the window swap for a rotation of ±90° or 270°.'],
      ['Is my video uploaded?', 'No. The crop runs on your Mac and the copy is written next to the original.'],
    ],
    related: [
      ['Crop image', '/tools/crop-image/'],
      ['Split video', '/tools/split-video/'],
      ['Join videos', '/tools/join-videos/'],
      ['Trim video', '/tools/trim-video/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'join-videos': {
    slug: 'join-videos',
    title: 'Join Videos on Mac for Free, Offline | Ferrocast',
    description: 'Join several video clips into one MP4 on your Mac and choose their order. Silent clips get generated silence. Needs the free ffmpeg.',
    h1: 'How to join videos on a Mac without uploading them',
    lead: 'Ferrocast joins several video clips into one MP4 on your Mac, in the order you choose. The first clip sets the canvas, and clips without sound get generated silence. Nothing is uploaded, and the clips you started with are never overwritten.',
    howToName: 'Join videos on a Mac with Ferrocast, without uploading them',
    stepsH: 'How to join videos on your Mac',
    steps: [
      ['Install Ferrocast and ffmpeg', `${INSTALL} Joining needs the free ffmpeg (\`brew install ffmpeg\`). Without it, Ferrocast shows that command rather than stopping with an error.`],
      ['Select the clips', 'Select several MP4, MOV or MKV files in Finder or in the launcher.'],
      ['Choose Join', 'Tap ⌘ while dragging the clips to open the Tools wheel, or open the Convert view, and choose Join.'],
      ['Put the clips in order', 'The Choose Order window lists the clips. ⌘↑ and ⌘↓ move the selected row, and dragging a row works too. The clips play in the order of the list.'],
      ['Open the MP4', 'Ferrocast writes one MP4 next to the clips. It never overwrites an existing file.'],
    ],
    sections: [
      {
        h: 'How the clips are combined',
        list: [
          'The result is one MP4, with the clips in the order of the Choose Order window.',
          'The first clip sets the canvas for the whole video, so put the clip whose size you want first.',
          'A clip with no sound gets generated silence.',
          'Input: MP4, MOV and MKV.',
        ],
      },
      {
        h: 'Joining from the keyboard',
        paras: [
          'Joining also works as a typed command. The [typed commands reference](/docs/files/typed-commands/) notes that Join accepts several clips and that silent clips get generated silence, exactly as in the window.',
        ],
      },
      {
        h: 'What it needs',
        paras: [
          'Join videos needs ffmpeg. Conversions use the tools that ship with macOS first; ffmpeg is one of five optional helpers that add audio and video work. If it is missing, Ferrocast shows `brew install ffmpeg` instead of failing silently.',
          'It is a one-step preset, so there is no timeline and no options beyond the order. To cut a clip first, use [Trim video](/tools/trim-video/) or [Split video](/tools/split-video/).',
        ],
      },
      {
        h: 'A common order of work',
        paras: [
          'Cut the clips you want first, with [Trim video](/tools/trim-video/), join them here, and crop the result with [Crop video](/tools/crop-video/) if the frame needs it. Each step writes a new copy next to its source, so the earlier files stay as they were.',
        ],
      },
      {
        h: 'Before you press join',
        paras: [
          'Check the order in the list: it is the order of the final video. If the clips have different sizes, remember that the first one decides the canvas.',
        ],
      },
      {
        h: 'Checking the result',
        paras: [
          'Play the MP4 from start to end once. The order is the one you set in the list, and the canvas is the first clip’s. If a clip is in the wrong place, run the tool again with the rows reordered: the first MP4 is not overwritten.',
        ],
      },
    ],
    faq: [
      ['Does joining videos need ffmpeg?', 'Yes. Join videos needs the free ffmpeg. Ferrocast shows the install command, `brew install ffmpeg`, when it is missing.'],
      ['What happens to clips that have no sound?', 'They get generated silence.'],
      ['Can I change the order of the clips?', 'Yes. The Choose Order window orders them with ⌘↑ and ⌘↓, or by dragging rows.'],
      ['Are my clips uploaded?', 'No. The clips are joined on your Mac and the MP4 is saved next to them.'],
    ],
    related: [
      ['Split video', '/tools/split-video/'],
      ['Trim video', '/tools/trim-video/'],
      ['Crop video', '/tools/crop-video/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'split-video': {
    slug: 'split-video',
    title: 'Split a Video on Mac for Free, Offline | Ferrocast',
    description: 'Split a video into clips on your Mac with cut markers or equal parts. Every clip goes into one new folder, nothing is uploaded. Needs ffmpeg.',
    h1: 'How to split a video on a Mac without uploading it',
    lead: 'Ferrocast cuts one video into several clips on your Mac, either at cut markers you place or into equal parts. Every clip is written into one new folder named after the file, and the original is never touched. Nothing is uploaded. It needs the free ffmpeg.',
    howToName: 'Split a video on a Mac with Ferrocast, without uploading it',
    stepsH: 'How to split a video on your Mac',
    steps: [
      ['Install Ferrocast and ffmpeg', `${INSTALL} Splitting needs the free ffmpeg (\`brew install ffmpeg\`); Ferrocast shows the command when the helper is not installed.`],
      ['Select the video', 'Pick an MP4, MOV or MKV file: drag it from Finder or select it in the launcher.'],
      ['Choose Split', 'Tap ⌘ while dragging to open the Tools wheel, or open the Convert view, and choose Split.'],
      ['Place the cuts', 'Press M to add a cut at the cursor and ⌫ to remove the last one. The arrow keys, Home, End and clicks move through the video.'],
      ['Or use equal parts', 'With no markers, the Equal parts box splits the video into 2 to 50 clips of the same length.'],
      ['Press ↵', 'Ferrocast writes every clip into one new folder next to the original.'],
    ],
    sections: [
      {
        h: 'Keys and buttons',
        paras: ['Every key has a mouse button above the bar:'],
        list: [
          '**Cut here** and **Remove last** add and remove cut markers.',
          '**◀ Frame** and **Frame ▶** step one frame at a time; **−1 s** and **+1 s** step one second.',
          'The other buttons are **Set start**, **Set end**, **To start** and **To end**.',
        ],
      },
      {
        h: 'Where the clips go',
        paras: [
          'The folder is named after the file, for example `<name> split`, and `split 2` if that name is already taken. The original is never touched. A split that would produce fewer than two clips is refused with a message.',
          'For a quick split without the window, type `split 30s` in the Convert view. The [typed commands reference](/docs/files/typed-commands/) lists the others.',
        ],
      },
      {
        h: 'Cuts land on keyframes',
        paras: [
          'Trim and split use stream copy: the video is copied, not re-encoded. That is quick, but a cut lands on a keyframe, so it can fall a little away from the exact frame you chose. Use [Trim video](/tools/trim-video/) to cut one range out of a clip.',
        ],
      },
      {
        h: 'An example with equal parts',
        paras: [
          'With 4 in the Equal parts box, a 40-minute recording becomes four clips of about 10 minutes each, written into one folder named after the file. With markers, the clips are as long as the gaps between your cuts.',
        ],
      },
    ],
    faq: [
      ['Does splitting a video need ffmpeg?', 'Yes. Split video needs the free ffmpeg. Install it with `brew install ffmpeg`, or let Ferrocast show the command.'],
      ['Will the cut be on the exact frame I choose?', 'Not always. Split uses stream copy, so the video is not re-encoded and each cut lands on a keyframe.'],
      ['Can I split a video into equal parts?', 'Yes. With no markers, the Equal parts box splits it into 2 to 50 clips. You can also type `split 30s`.'],
      ['Is my video uploaded?', 'No. The clips are written to a new folder on your Mac, next to the original.'],
    ],
    related: [
      ['Trim video', '/tools/trim-video/'],
      ['Join videos', '/tools/join-videos/'],
      ['Crop video', '/tools/crop-video/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'trim-audio': {
    slug: 'trim-audio',
    title: 'Trim Audio on Mac for Free, Offline | Ferrocast',
    description: 'Trim an audio file on your Mac on a waveform with exact start and end boxes, or cut silence at the ends. Needs ffmpeg. Nothing is uploaded.',
    h1: 'How to trim audio on a Mac without uploading it',
    lead: 'Ferrocast trims an audio file on your Mac. The trim window shows the waveform with markers, exact start and end boxes and a Trim silence button that removes silence at the ends, and it saves the result as a new copy next to the original. Nothing is uploaded. It needs the free ffmpeg.',
    howToName: 'Trim audio on a Mac with Ferrocast, without uploading it',
    stepsH: 'How to trim audio on your Mac',
    steps: [
      ['Install Ferrocast and ffmpeg', `${INSTALL} Trimming audio needs the free ffmpeg (\`brew install ffmpeg\`), and Ferrocast prints that command if the helper is missing.`],
      ['Select the audio file', 'Drag an MP3, M4A, WAV or FLAC file from Finder, or select it in the launcher.'],
      ['Choose Trim', 'Tap ⌘ while dragging to open the Tools wheel, or open the Convert view, and choose Trim.'],
      ['Set the range', 'Move the markers on the waveform, or type the exact start and end in the boxes.'],
      ['Or cut the silence', 'Press Trim silence to remove the silence at the beginning and the end of the recording.'],
      ['Save the copy', 'Ferrocast writes a new file next to the original, with “trimmed” in its name. It never overwrites a file.'],
    ],
    sections: [
      {
        h: 'What you can set',
        list: [
          'Markers on the waveform for the start and the end.',
          'Exact start and end boxes, for a range you know to the second.',
          'A Trim silence button, which removes silence at the ends of the file.',
          'Input: MP3, M4A, WAV and FLAC.',
        ],
      },
      {
        h: 'Trim with a typed command',
        paras: [
          'Typed commands skip the window. In the Convert view for an audio file, type `trim 0:10-0:30` to keep the range from 0:10 to 0:30, or `silence` to trim the silence at the ends. See the [typed commands reference](/docs/files/typed-commands/) for the rest.',
        ],
      },
      {
        h: 'After trimming',
        paras: [
          'If the recording is too quiet or too loud, [Normalize audio](/tools/normalize-audio/) sets its loudness in LUFS and shows the measured loudness before and after. To cut a long recording into several files, type `split 30s` in the Convert view: the split command works on audio or video.',
        ],
      },
      {
        h: 'Limits',
        list: [
          'Trim audio needs ffmpeg, which you install once with Homebrew.',
          'The silence threshold is measured per sample, so a single click in a recording can shift where the silence is cut.',
          'There is no before and after comparison bar: listen to the copy before you delete anything.',
        ],
      },
      {
        h: 'An example range',
        paras: [
          'The typed range `trim 0:10-0:30` keeps 20 seconds of the file, from ten seconds in to thirty seconds in. The same range can be set in the window with the start and end boxes, and the markers on the waveform follow what you type.',
        ],
      },
    ],
    faq: [
      ['Does trimming audio need ffmpeg?', 'Yes. Trim audio needs the free ffmpeg; Ferrocast shows `brew install ffmpeg` when it is missing.'],
      ['Can I remove the silence at the start and end of a recording?', 'Yes. Press Trim silence in the window, or type `silence` in the Convert view.'],
      ['Does trimming overwrite my recording?', 'No. Ferrocast saves a new copy next to the original.'],
      ['Is my audio uploaded?', 'No. The trim runs on your Mac.'],
    ],
    related: [
      ['Normalize audio', '/tools/normalize-audio/'],
      ['Trim video', '/tools/trim-video/'],
      ['Split video', '/tools/split-video/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'edit-metadata': {
    slug: 'edit-metadata',
    title: 'Edit File Metadata on Mac for Free, Offline | Ferrocast',
    description: 'Inspect, edit or remove metadata of audio, video, GIF and PDF files on your Mac, grouped by file, track and chapter. Nothing is uploaded.',
    h1: 'How to edit file metadata on a Mac without uploading it',
    lead: 'Ferrocast opens one window to inspect, edit or remove the metadata of audio, video, GIF and PDF files on your Mac, with fields grouped by file, track and chapter. Images use short typed commands instead. Nothing is uploaded, and the result is a copy next to the original.',
    howToName: 'Edit file metadata on a Mac with Ferrocast, without uploading the file',
    stepsH: 'How to edit file metadata on your Mac',
    steps: [
      ['Install Ferrocast', `${INSTALL}`],
      ['Select the file', 'Drag an audio, video, GIF or PDF file from Finder, or select it in the launcher.'],
      ['Choose Edit metadata', 'Tap ⌘ while dragging to open the Tools wheel, or open the Convert view, and choose Edit metadata.'],
      ['Edit the fields', 'Type in the text boxes, or press the clear button of a row to empty it. Technical keys are read-only.'],
      ['Or remove everything', 'Turn on Remove all metadata. A counter shows how many changes you have made.'],
      ['Save the copy', 'Ferrocast writes a copy next to the original, with “edited” in its name. It never overwrites a file.'],
    ],
    sections: [
      {
        h: 'What you can set',
        list: [
          'Editable text boxes, grouped by file, track and chapter, each with a clear button.',
          'Read-only technical keys, which you can see but not change.',
          'A Remove all metadata toggle and a counter of your changes.',
          'Input: audio (MP3, M4A, WAV, FLAC), video (MP4, MOV, MKV), GIF and PDF, plus JPEG, PNG, WebP, HEIC and TIFF images.',
        ],
      },
      {
        h: 'Images: typed commands',
        paras: [
          'Images have no window. In the Convert view, type `make Canon`, `model EOS R5` or `gps 40.4,-3.7` to write camera and GPS tags. These work on JPEG; PNG tags are not supported and Ferrocast shows a clear error.',
          'Removing metadata from JPEG and PNG images is lossless. For the step-by-step privacy case, see the guide to [removing location and metadata on a Mac](/remove-metadata-mac/).',
        ],
      },
      {
        h: 'Look before you change anything',
        paras: [
          'The window is also an inspector. Fields are grouped by file, track and chapter, so you can read what a file stores before you edit or remove any of it, and the counter shows how many changes you have made so far.',
        ],
      },
      {
        h: 'Limits',
        list: [
          'Chapters are read-only.',
          'A GIF can only be stripped, not edited field by field.',
          'Remove all leaves some container tags, so check the copy before you share it.',
          'Camera and GPS tags are written for JPEG; PNG tags are not supported.',
        ],
      },
      {
        h: 'Before you share a file',
        paras: [
          'Open the copy in the window once more after you save it, and read the fields again. That is the quickest way to see what the file still carries, especially after Remove all metadata, which leaves some container tags.',
        ],
      },
    ],
    faq: [
      ['Does Remove all metadata erase everything?', 'Not always. Remove all leaves some container tags, so look at the copy before you send it.'],
      ['Can I change the GPS location of a photo?', 'For JPEG, yes: type `gps 40.4,-3.7` in the Convert view. PNG tags are not supported.'],
      ['Does editing metadata overwrite my file?', 'No. Ferrocast writes a copy next to the original and never overwrites it.'],
      ['Is my file uploaded?', 'No. Everything happens on your Mac, and Ferrocast runs on Apple Silicon only.'],
    ],
    related: [
      ['Remove location and metadata on a Mac', '/remove-metadata-mac/'],
      ['Compress files', '/tools/compress/'],
      ['Crop image', '/tools/crop-image/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'edit-photos': {
    slug: 'edit-photos',
    title: 'Edit Photos on Mac for Free, Offline | Ferrocast',
    description: 'Edit photos on your Mac with a live preview and nine sliders: brightness, contrast, saturation, warmth and more. Free, nothing uploaded.',
    h1: 'How to edit photos on a Mac without uploading them',
    lead: 'Ferrocast edits a photo on your Mac with a live preview and nine sliders: brightness, contrast, saturation, warmth, clarity, sharpen, grain, denoise and dehaze. Press ↵ and the full-size copy is saved next to the original. Nothing is uploaded and the photo is never overwritten.',
    howToName: 'Edit a photo on a Mac with Ferrocast, without uploading it',
    stepsH: 'How to edit photos on your Mac',
    steps: [
      ['Install Ferrocast', INSTALL],
      ['Select the photo', 'Choose a photo to edit: a JPEG, PNG, WebP, HEIC or TIFF file, dragged from Finder or selected in the launcher.'],
      ['Open the editor', 'Press ⇧⌘E on the image result, or tap ⌘ while dragging to open the Tools wheel and choose Edit photo.'],
      ['Move the sliders', 'Change any of the nine sliders. The preview is recomputed as you go.'],
      ['Press ↵', 'Ferrocast writes the edited copy at full size next to the original. It never overwrites a file.'],
    ],
    sections: [
      {
        h: 'The nine sliders',
        list: [
          'Brightness, contrast, saturation and warmth.',
          'Clarity and sharpen.',
          'Grain, denoise and dehaze.',
        ],
      },
      {
        h: 'Presets and the typed command',
        paras: [
          'Four one-step presets apply without opening the window: black & white, brighter, darker and more contrast.',
          'The typed command sets every slider at once. In the Convert view, type `edit brightness 20 contrast 10 saturation -30 warmth 15 clarity 40 sharpen 30 grain 20 denoise 30 dehaze 50` and press ↵. The other image commands are in the [typed commands reference](/docs/files/typed-commands/).',
        ],
      },
      {
        h: 'Try again from the original',
        paras: [
          'Nothing is overwritten, so you can open the same photo again with different settings. Each result is a separate file, and the original is always there to start from.',
        ],
      },
      {
        h: 'Preview versus saved copy',
        paras: [
          'The preview is computed on a reduced copy of the photo, so it stays fast. The copy that ↵ writes is full size. Look at the saved file at 100% before you rely on a subtle setting such as sharpen or denoise.',
        ],
      },
      {
        h: 'A common order of work',
        paras: [
          'Crop the photo first with [Crop image](/tools/crop-image/), then adjust it here, and finish with [Add a background](/tools/add-background/) if you want it on a canvas. Each step saves its own copy beside the one before.',
        ],
      },
      {
        h: 'Other ways to edit a photo on a Mac',
        paras: [
          `The Photos app has its own editing tools. Apple’s Photos guide describes the Adjust panel, with groups of sliders for light, colour and black & white ([Apple’s Photos guide](${PHOTOS})). Ferrocast works on a file you already have in a folder and writes the edited copy next to it.`,
        ],
      },
      {
        h: 'Presets or sliders',
        paras: [
          'The four presets are one-step fixes that need no window: black & white, brighter, darker and more contrast. Use the sliders when you want to combine several changes in one pass, such as a little more contrast with less saturation and some sharpen.',
        ],
      },
    ],
    faq: [
      ['Is the saved photo full size?', 'Yes. The preview uses a reduced copy, but pressing ↵ writes the full-size copy.'],
      ['Will editing overwrite the original photo?', 'No. Ferrocast saves a copy next to the original and never overwrites it.'],
      ['Can I edit a photo without opening a window?', 'Yes. Use the typed `edit` command, or one of the four presets: black & white, brighter, darker, more contrast.'],
      ['Are my photos uploaded?', 'No. Everything runs on your Mac, which must be an Apple Silicon Mac.'],
    ],
    related: [
      ['Crop image', '/tools/crop-image/'],
      ['Add a background to an image', '/tools/add-background/'],
      ['Annotate an image', '/tools/annotate/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },

  'add-background': {
    slug: 'add-background',
    title: 'Add a Background to an Image on Mac for Free | Ferrocast',
    description: 'Place an image on a colour, a blurred copy or another picture on your Mac, with padding, rounded corners and a shadow. Nothing is uploaded.',
    h1: 'How to add a background to an image on a Mac without uploading it',
    lead: 'Ferrocast places an image on a colour, a blurred copy of itself or another picture on your Mac, with padding, rounded corners and a shadow. The editor previews the result live, and ↵ saves a full-size copy next to the original. Nothing is uploaded and the image is never overwritten.',
    howToName: 'Add a background to an image on a Mac with Ferrocast, without uploading it',
    stepsH: 'How to add a background to an image on your Mac',
    steps: [
      ['Install Ferrocast', INSTALL],
      ['Select the image', 'Choose the picture to frame: a JPEG, PNG, WebP, HEIC or TIFF file, dragged from Finder or selected in the launcher.'],
      ['Open the background editor', 'Press ⇧⌘B on the image result, or tap ⌘ while dragging to open the Tools wheel and choose Add a background.'],
      ['Choose the backdrop', 'Pick Colour, Blur or Image. For Image, press Choose image… and select the picture to use behind yours.'],
      ['Set padding, corners and shadow', 'Adjust the padding around the image, the rounded corners and the shadow. The preview updates as you change them.'],
      ['Press ↵', 'The composed image is saved at full size, next to the original, and no existing file is overwritten.'],
    ],
    sections: [
      {
        h: 'What you can set',
        list: [
          'Backdrop: a colour, a blurred copy of the image, or another image.',
          'Padding around the image.',
          'Rounded corners and a shadow.',
          'A square white or black background is also available as a one-step preset.',
        ],
      },
      {
        h: 'Add a background with a typed command',
        paras: [
          'Typed commands skip the window. In the Convert view, type `frame blur 40 pad 80 shadow 30` for a blurred backdrop with padding and a shadow, or `frame image /path/bg.jpg` to use another picture as the background. More commands are in the [typed commands reference](/docs/files/typed-commands/).',
        ],
      },
      {
        h: 'Preview versus saved copy',
        paras: [
          'The live preview is composed on a reduced copy of the image so it stays quick while you change the settings. What ↵ saves is built from the full-size image.',
        ],
      },
      {
        h: 'What each backdrop is',
        list: [
          '**Colour:** a flat colour behind the image.',
          '**Blur:** a blurred copy of the same image behind it.',
          '**Image:** another picture you choose with Choose image….',
        ],
      },
      {
        h: 'A common order of work',
        paras: [
          'Crop and adjust the picture first, then add the background last, so the padding and the shadow are drawn around the final image. Each step writes its own copy next to the original.',
        ],
      },
      {
        h: 'Related image tools',
        paras: [
          'Need to cut the picture first? [Crop image](/tools/crop-image/) comes before this step. To put several pictures on one canvas, use [Photo collage](/tools/collage/).',
        ],
      },
      {
        h: 'Padding, corners and shadow together',
        paras: [
          'The three settings work together: padding sets how much backdrop shows around the image, the rounded corners soften the image’s edges and the shadow lifts it off the backdrop. The typed example `frame blur 40 pad 80 shadow 30` sets a blur, padding and a shadow in one line.',
        ],
      },
    ],
    faq: [
      ['Can I use my own picture as the background?', 'Yes. Choose Image and press Choose image…, or type `frame image /path/bg.jpg` in the Convert view.'],
      ['Can I get a blurred background from the same photo?', 'Yes. Choose Blur in the editor, or type `frame blur 40` with the values you like.'],
      ['Does it overwrite the original image?', 'No. Ferrocast saves a full-size copy next to the original.'],
      ['Is my image uploaded?', 'No. The background is composed on your Mac, and Ferrocast needs an Apple Silicon Mac.'],
    ],
    related: [
      ['Crop image', '/tools/crop-image/'],
      ['Edit photos', '/tools/edit-photos/'],
      ['Photo collage', '/tools/collage/'],
      ['Free file converter for Mac', '/free-file-converter-mac/'],
    ],
  },
};
