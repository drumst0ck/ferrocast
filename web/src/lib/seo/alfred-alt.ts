/*
 * Copy for /alfred-alternative/ (English, board idea "Free Alfred alternative (clipboard + snippets
 * without Powerpack)"). Alfred facts come only from alfredapp.com (powerpack, shop, clipboard help,
 * minimum requirements), read on CHECKED. Ferrocast facts come from its docs (clipboard, snippets,
 * script-commands, settings). Rules: no speed or memory numbers for Alfred, never "Alfred does not
 * have" without a source, and say plainly who should stay on Alfred.
 */
import type { GuideCopy } from './compress-video';

export const CHECKED = '2026-10-27';

export const page: GuideCopy = {
  lang: 'en',
  path: '/alfred-alternative/',
  title: 'Free Alfred alternative: clipboard and snippets, no Powerpack | Ferrocast',
  description:
    'Ferrocast is a free, open-source Mac launcher with clipboard history and expanding snippets built in, the two features Alfred keeps in its paid Powerpack.',
  crumbs: [['Home', '/'], ['Alfred alternative', '/alfred-alternative/']],
  h1: 'A free Alfred alternative with clipboard history and snippets',
  lead: [
    'In Alfred, clipboard history, snippets with auto-expansion and Workflows are part of the Powerpack, a one-off purchase (£34 for a single Alfred 5 licence, which the shop says includes a free upgrade to Alfred 6). Ferrocast includes clipboard history and expanding snippets for free, as an open-source (MIT) launcher for Apple Silicon Macs.',
    'Alfred has a decade of Workflows behind it and runs on older Macs; Ferrocast does not try to replace that. If what you want from the Powerpack is the clipboard and snippets, read on. I make Ferrocast, so check the sources.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Alfred Powerpack features and their Ferrocast counterpart',
    caption: 'Alfred Powerpack features compared with what Ferrocast includes for free',
    cols: ['Feature', 'Alfred', 'Ferrocast (free)'],
    rows: [
      ['Price', 'Free app; Powerpack is a one-off £34 (single Alfred 5 licence)', 'Free, no paid plan'],
      ['Clipboard history', 'Powerpack; ignores password apps by default', 'Included; skips entries from password managers; you choose how long to keep it'],
      ['Snippets with auto-expansion', 'Powerpack', 'Included, with placeholders such as `{date}`, `{clipboard}` and `{cursor}`; imports Raycast snippet JSON'],
      ['Automation', 'Workflows (Powerpack), with thousands from the community', 'Raycast-compatible [Script Commands](/raycast-script-commands/); no visual workflow editor'],
      ['Shell commands', 'Run in Terminal from Alfred (Powerpack)', 'Script Commands run without opening Terminal'],
      ['Settings sync between Macs', 'Through Dropbox or another file sync service (Powerpack)', 'Opt-in through iCloud Drive (Settings → Advanced, off by default). Keychain API keys stay on each Mac. Backup export is still in Settings'],
      ['AI', 'No built-in AI stated on its website', 'Ask AI with your provider; Ollama on your Mac by default'],
      ['File conversion', 'File actions, and custom ones through Workflows', '275 local conversions and 26 file tools'],
      ['Window management', 'Not stated on its website', 'About 40 commands'],
      ['Requirements', 'Alfred 5: macOS 10.14 or later', 'Apple Silicon, macOS 13 or later'],
      ['Source code', 'No open licence stated', 'MIT'],
    ],
    note: 'Alfred column from [alfredapp.com/powerpack](https://www.alfredapp.com/powerpack/), [the shop](https://www.alfredapp.com/shop/), [clipboard help](https://www.alfredapp.com/help/features/clipboard/) and [minimum requirements](https://www.alfredapp.com/help/getting-started/minimum-requirements/), checked 27 October 2026. Ferrocast column from its [key facts](/facts/) and docs.',
  },
  steps: {
    h: 'Move your clipboard and snippets habit to Ferrocast',
    howToName: 'Set up clipboard history and snippets in Ferrocast',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew. It needs no account.'],
      ['Pick a hotkey', 'Ferrocast opens with ⌥ Space by default; change it in Settings if Alfred uses it.'],
      ['Turn on the clipboard history', 'In Settings → Clipboard, keep Record history on and choose how long to keep it.'],
      ['Recreate your snippets', 'Open Snippets from the launcher, press ⌘N and type the keyword and text. Automatic expansion needs the Accessibility permission.'],
    ],
  },
  sections: [
    {
      h: 'Stay on Alfred if you…',
      paras: [],
      list: [
        'rely on Workflows, your own or from the community;',
        'use an Intel Mac or a macOS version older than 13;',
        'want Alfred 6, which the shop says is a free upgrade for new Alfred 5 Powerpack buyers;',
        'want settings sync through Dropbox rather than iCloud Drive.',
      ],
    },
    {
      h: 'Ferrocast fits if you…',
      paras: [],
      list: [
        'mostly want clipboard history and snippets without paying for them;',
        'want AI chat with your own model, file conversion that never uploads, or screen recording, in the same launcher;',
        'prefer software whose code you can read.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is Alfred free?', 'The Alfred app is free. Clipboard history, snippets, Workflows and settings sync are part of the Powerpack, a one-off purchase (£34 for a single Alfred 5 licence when we checked on 27 October 2026).'],
      ['Is there a free alternative to the Alfred Powerpack?', 'For clipboard history and snippets, yes. Ferrocast is a free, open-source Mac launcher that includes both. It does not run Alfred Workflows.'],
      ['Can Ferrocast import my Alfred snippets?', 'Not directly. Ferrocast imports snippets from a Raycast JSON export; Alfred snippets have to be recreated.'],
      ['Does Ferrocast run Alfred Workflows?', 'No. Ferrocast runs Raycast-compatible Script Commands, which are plain scripts with metadata comments.'],
      ['Does Ferrocast work on Intel Macs?', 'No. Ferrocast needs an Apple Silicon Mac and macOS 13 or later. Alfred 5 runs from macOS 10.14.'],
      ['Is Ferrocast affiliated with Alfred?', 'No. Ferrocast is independent and not affiliated with Alfred or Raycast.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Free clipboard manager for Mac', '/clipboard-manager-mac/'],
      ['Free text expander for Mac', '/text-expander-mac/'],
      ['Clipboard history docs', '/docs/launcher/clipboard/'],
      ['Snippets docs', '/docs/launcher/snippets/'],
      ['Run Raycast Script Commands', '/raycast-script-commands/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['Key facts', '/facts/'],
    ],
  },
};
