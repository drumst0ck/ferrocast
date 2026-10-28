/*
 * Copy for /cleanshot-alternative/ (English only, board idea "Free CleanShot X alternative inside your
 * launcher"). Ferrocast facts come from docs/launcher/screen-capture.md, docs/files/tools/annotate.mdx and
 * crates/macos/src/scroll.rs (vertical scroll events only). CleanShot facts come only from
 * cleanshot.com/pricing and cleanshot.com/features, read on 28 October 2026. What those pages do not state
 * is written as "not stated" (never "no"). Rules: no "replaces CleanShot", no speed, quality or memory
 * comparison, no CleanShot Product/Offer markup, sync does not apply here.
 */
import type { GuideCopy } from './compress-video';

export const CHECKED = '2026-10-28';

const PRICING = 'https://cleanshot.com/pricing';
const FEATURES = 'https://cleanshot.com/features';

export const page: GuideCopy = {
  lang: 'en',
  path: '/cleanshot-alternative/',
  title: 'A Free CleanShot X Alternative for Mac, Open Source | Ferrocast',
  description:
    'Scrolling screenshots, copy text, pin, annotate and record with auto zoom on your Mac. Free and MIT. What CleanShot X does that Ferrocast does not.',
  crumbs: [['Home', '/'], ['CleanShot X alternative', '/cleanshot-alternative/']],
  h1: 'A free CleanShot X alternative for Mac',
  lead: [
    'Ferrocast is a free, open-source Mac launcher that also takes screenshots, including scrolling captures, copies the text in a capture on your Mac, pins captures above your windows, annotates them and records your screen with automatic zoom. It has no cloud upload, no share links and no capture history, and screen capture needs macOS 14.',
    'CleanShot X is a polished paid app and this page does not claim to replace everything it does. According to its site (28 October 2026) CleanShot Basic is $35 once, with one year of updates, and Pro is $10 per user per month billed annually. I build Ferrocast; it is independent and not affiliated with CleanShot X.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · capture needs macOS 14' },
  limits: {
    h: 'What you get, feature by feature',
    caption: 'Ferrocast and CleanShot X compared feature by feature',
    cols: ['', 'Ferrocast', 'CleanShot X'],
    rows: [
      ['Price', 'Free (MIT)', '$35 one-time with 1 year of updates, then an optional $19 a year; or Pro at $10 per user per month billed annually'],
      ['Trial', 'Free, nothing to trial', 'No trial; 30-day money-back guarantee; Setapp has a 7-day trial'],
      ['Area, window and full screen', 'Yes', 'Yes'],
      ['Scrolling capture', 'Yes, vertical; stops when the content stops moving or after 60 screens', 'Yes, vertical and horizontal'],
      ['Copy text from an image', 'Yes, on your Mac with Apple’s Vision framework', 'Yes, on-device; lists 30+ languages and QR codes'],
      ['Pin a screenshot', 'Yes, always on top, zoom 25–400 %, several at once', 'Yes, always on top, size and opacity, lock mode'],
      ['Annotate', 'Arrows, lines, boxes, ellipses, highlights, freehand, numbered labels, magnifier, text, callouts, image stamps', 'Also lists pixelate, blur and spotlight'],
      ['Screen recording', 'MP4 or GIF, auto zoom on clicks, camera, microphone and system audio, on-device transcript', 'MP4 or GIF, clicks, keystrokes, camera; editor with smart zooms'],
      ['Cloud and share links', 'No', 'Yes (1 GB on Basic, unlimited on Pro)'],
      ['Capture history', 'Not documented; captures are PNG files in a folder', 'Up to 1 month'],
      ['macOS', 'Capture needs macOS 14; Apple Silicon only', 'macOS 13 or newer'],
    ],
    note: `Ferrocast: [screen capture docs](/docs/launcher/screen-capture/) and [Annotate docs](/docs/files/tools/annotate/). CleanShot X: [${PRICING.replace('https://', '')}](${PRICING}) and [${FEATURES.replace('https://', '')}](${FEATURES}). Checked on 28 October 2026.`,
  },
  steps: {
    h: 'Take a scrolling screenshot and copy its text',
    howToName: 'Take a scrolling screenshot and copy its text on a Mac with Ferrocast',
    items: [
      ['Install and allow permissions', 'Install Ferrocast, then allow Screen Recording; scrolling capture also needs Accessibility. There are no default shortcuts: set them in Settings → Capture → Shortcuts.'],
      ['Run Capture Scrolling', 'Open the launcher with ⌥ Space, run Capture Scrolling and choose the area that scrolls. Ferrocast scrolls it and joins the strips.'],
      ['Use the Capture window', 'Copy (↵), Save (⌘S), Annotate… (⌘⇧A), Background… (⌘⇧B), Copy Text (⌘T) or Pin (⌘P).'],
      ['Copy the text', 'Press ⌘T. Ferrocast reads the text with Vision on your Mac and shows “Copied N characters”.'],
    ],
  },
  sections: [
    {
      h: 'Screenshots',
      paras: [
        'Capture Area shows a crosshair and the size in pixels (Space switches to window mode), Capture Window and Capture Screen do what they say. Captures are PNG files at the display’s native size in `~/Pictures/Ferrocast Captures`.',
        'Capture Scrolling stops when two strips are identical or after 60 screens. Ferrocast sends vertical scroll events, so I only claim vertical capture; CleanShot also lists horizontal scrolling.',
      ],
    },
    {
      h: 'Copy text, pin and annotate',
      paras: [
        'Copy Text puts the text on the clipboard in reading order; the image is not sent anywhere. Which languages it reads and whether it reads QR codes are not documented, so I do not compare them with CleanShot’s 30+ languages.',
        'Pin opens a borderless always-on-top window; Esc closes it. For hiding sensitive data, Ferrocast has [Redact image](/tools/redact-image/) for faces and text with blur or a solid colour; I have not documented a direct route to it from the Capture window.',
      ],
    },
    {
      h: 'Record your screen with auto zoom',
      paras: [
        'Record Screen, Area or Window at 60 or 30 fps. Auto zoom starts 0.4 s before the first click of a group and returns after 1.2 s without clicks or cursor movement (2× by default). Export is MP4 (H.264 or HEVC, up to 4K at 60 fps) or GIF. Ferrocast logs shortcut keys, never plain typing; CleanShot lists capturing all keys or only command keys, so I do not compare the two. More in the [Screen Studio alternative page](/screen-studio-alternative/) ([en español](/es/grabar-pantalla-mac-zoom-automatico/)).',
      ],
    },
    {
      h: 'What CleanShot X does that Ferrocast doesn’t',
      list: [
        'Cloud upload and share links, with a searchable library on its Cloud plans.',
        'A capture history of up to a month, and a Quick Access Overlay after every capture.',
        'A self-timer for screenshots; Ferrocast documents a countdown for recordings only.',
        'Horizontal scrolling capture.',
        'macOS 13 support: Ferrocast capture needs macOS 14.',
        'Pixelate and spotlight tools in its annotation editor.',
      ],
    },
    {
      h: 'Price',
      paras: [
        'Ferrocast is free, with no paid plan. CleanShot X costs $35 (Basic, one year of updates, $19 a year to renew) or $10 per user per month billed annually (Pro), according to [cleanshot.com/pricing](https://cleanshot.com/pricing) on 28 October 2026.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is there a free alternative to CleanShot X?', 'Yes. Ferrocast is a free, open-source (MIT) Mac app that takes area, window, screen and scrolling screenshots, copies the text in a capture on your Mac, pins captures above other windows, annotates them and records the screen with automatic zoom on clicks. It has no cloud upload or capture history, and screen capture needs macOS 14 or later.'],
      ['How much does CleanShot X cost?', 'According to cleanshot.com (28 October 2026), CleanShot Basic is a $35 one-time purchase with one year of updates, renewable at $19 a year, and CleanShot Pro is $10 per user per month billed annually. There is no free trial, but there is a 30-day money-back guarantee.'],
      ['Can I copy text from a screenshot on a Mac without uploading it?', 'Yes. In Ferrocast’s Capture window, Copy Text (⌘T) reads the text with Apple’s Vision framework on your Mac and puts it on the clipboard. The image is not sent anywhere.'],
      ['Can I take a scrolling screenshot for free?', 'Yes. Capture Scrolling in Ferrocast scrolls the area you choose and joins the strips into one image, stopping when the content stops moving or after 60 screens. It is vertical only and needs the Accessibility permission.'],
      ['Can I pin a screenshot above other windows?', 'Yes. Pin (⌘P) opens the capture in a borderless always-on-top window that you can zoom from 25 % to 400 %; you can have several at once and Esc closes one.'],
      ['Can I record the screen with automatic zoom?', 'Yes. Ferrocast zooms toward your clicks, starting 0.4 seconds before the first click of a group, and exports MP4 or GIF. It is free.'],
      ['Which macOS and Macs does it need?', 'Screen capture needs macOS 14 or later and an Apple Silicon Mac; Intel Macs are not supported. CleanShot X lists macOS 13 or newer on its pricing page.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Screen capture docs', '/docs/launcher/screen-capture/'],
      ['Free Screen Studio alternative', '/screen-studio-alternative/'],
      ['Grabar la pantalla con zoom automático', '/es/grabar-pantalla-mac-zoom-automatico/'],
      ['Redact image', '/tools/redact-image/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['Key facts', '/facts/'],
    ],
  },
};
