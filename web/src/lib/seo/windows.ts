/*
 * Copy for /mac-window-management/ (English, board idea "Window management shortcuts on Mac without
 * Rectangle (40 commands in the launcher)"). The command list is crates/macos/src/windows.rs; the
 * built-in macOS tiling facts come from Apple Support; Rectangle facts only from rectangleapp.com,
 * read on 2026-10-27 (its licence is not stated there, so it is not stated here).
 */
import type { GuideCopy } from './compress-video';

/** The window commands, in the order windows.rs defines them. A test checks them against the source. */
export const COMMANDS = [
  'Left Half', 'Right Half', 'Top Half', 'Bottom Half',
  'Top Left Quarter', 'Top Right Quarter', 'Bottom Left Quarter', 'Bottom Right Quarter',
  'First Third', 'Center Third', 'Last Third', 'First Two Thirds', 'Last Two Thirds',
  'First Fourth', 'Second Fourth', 'Third Fourth', 'Last Fourth',
  'Top Left Sixth', 'Top Center Sixth', 'Top Right Sixth', 'Bottom Left Sixth', 'Bottom Center Sixth', 'Bottom Right Sixth',
  'Maximize', 'Almost Maximize', 'Maximize Width', 'Maximize Height', 'Center', 'Reasonable Size',
  'Move Up', 'Move Down', 'Move Left', 'Move Right', 'Make Larger', 'Make Smaller', 'Restore',
  'Next Display', 'Previous Display', 'Toggle Fullscreen', 'Minimize',
] as const;

const TILING = 'https://support.apple.com/guide/mac-help/mac-window-tiling-icons-keyboard-shortcuts-mchl9674d0b0/mac';

export const page: GuideCopy = {
  lang: 'en',
  path: '/mac-window-management/',
  title: 'Mac window management from your launcher: 40 commands | Ferrocast',
  description: `Move and resize Mac windows from the keyboard with ${COMMANDS.length} commands (halves, thirds, quarters, sixths, displays) in a free, open-source launcher.`,
  crumbs: [['Home', '/'], ['Window management', '/mac-window-management/']],
  h1: `${COMMANDS.length} window commands, in the launcher you already use`,
  lead: [
    `Ferrocast has ${COMMANDS.length} commands that move and resize the frontmost window: halves, thirds, fourths, quarters, sixths, maximize, center, nudge, and moving to another display. Run one by typing its name in the launcher, or give it a hotkey with ⇧⌘, on the command. It is free and open source (MIT).`,
    `You may not need another app at all: macOS has built-in tiling with the green button, the Window menu and Fn-Control-arrow shortcuts ([Apple](${TILING})). And Rectangle is a dedicated window manager that is free to download, with a paid Pro version. Ferrocast is for people who would rather have it in the launcher.`,
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Three ways to manage windows on a Mac',
    caption: 'macOS tiling, Rectangle and Ferrocast compared',
    cols: ['', 'macOS (built in)', 'Rectangle', 'Ferrocast'],
    rows: [
      ['How you use it', 'Green button, Window menu, Fn-Control shortcuts, dragging to screen edges', 'Keyboard shortcuts or snap areas', 'Type the command in the launcher, or give it a hotkey'],
      ['Layouts', 'Halves, quarters, fill, center, arrangements of several windows', 'Not compared here', 'Halves, thirds, fourths, quarters, sixths, maximize, center, nudge, displays'],
      ['Price', 'Included', 'Free download; Rectangle Pro is paid, with a 10-day trial', 'Free'],
      ['Also does', '—', 'Window management', 'Launcher, clipboard, snippets, file conversion, screen recording, AI chat'],
    ],
    note: `macOS from [Apple Support](${TILING}); Rectangle from [rectangleapp.com](https://rectangleapp.com/), checked 27 October 2026.`,
  },
  steps: {
    h: 'Set it up',
    howToName: 'Manage Mac windows from the Ferrocast launcher',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew.'],
      ['Grant Accessibility', 'Window commands need the Accessibility permission; Ferrocast asks on first run.'],
      ['Run a command', 'Open the launcher with ⌥ Space and type, for example, “left half” or “next display”.'],
      ['Add hotkeys', 'Select a command and press ⇧⌘, to give it its own shortcut.'],
    ],
  },
  sections: [
    {
      h: 'All the commands',
      paras: [COMMANDS.join(' · ') + '.'],
    },
    {
      h: 'Limits',
      list: [
        'Needs the Accessibility permission.',
        'No snap areas: dragging a window to an edge does not tile it (macOS does that on its own).',
        'Automatic window tiling is included and off by default (Settings → Advanced → Window tiling). A new window is placed on its display in one of four layouts. It pauses while Stage Manager is on. The on/off switch and each display’s layout stay on that Mac. See the [tiling reference](/docs/launcher/tiling/).',
        'Apple Silicon Macs only.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Can I manage windows on a Mac without Rectangle?', 'Yes. macOS has built-in tiling with the green button, the Window menu and Fn-Control-arrow shortcuts, and Ferrocast adds 40 window commands to its launcher.'],
      ['Which window layouts does Ferrocast have?', 'Halves, thirds, two thirds, fourths, quarters and sixths, plus maximize, almost maximize, maximize width or height, center, reasonable size, move, make larger or smaller, restore, next or previous display, full screen and minimize.'],
      ['Can I set my own shortcuts?', 'Yes. Select a window command in the launcher and press ⇧⌘, to give it a hotkey.'],
      ['Is Ferrocast free?', 'Yes. Ferrocast is free and open source under the MIT licence.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Window management docs', '/docs/launcher/window-management/'],
      ['En español: gestión de ventanas', '/es/gestion-de-ventanas-mac/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['Ferrocast vs Raycast', '/ferrocast-vs-raycast/'],
      ['Key facts', '/facts/'],
    ],
  },
};
