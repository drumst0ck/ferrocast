/*
 * Copy for /ferrocast-vs-raycast/ (English): "Is Raycast free?" and Raycast Free vs Pro vs Ferrocast.
 * Source: the Notion brief "Brief: Ferrocast vs Raycast (EN)". Raycast facts come only from
 * raycast.com/pricing and manual.raycast.com/billing, read on CHECKED; re-check prices every quarter.
 * Ferrocast facts come from the docs (ask-ai, translate, notes, clipboard, window-management).
 * Rules: no speed or memory numbers for Raycast, never "Raycast does not have" without a source,
 * and Ferrocast's gaps stay in the table.
 */
import type { GuideCopy } from './compress-video';

export const CHECKED = '2026-10-27';

export const page: GuideCopy = {
  lang: 'en',
  path: '/ferrocast-vs-raycast/',
  title: 'Ferrocast vs Raycast: is Raycast free, and what Pro adds | Ferrocast',
  description:
    'Raycast Free vs Raycast Pro vs Ferrocast, feature by feature: AI, sync, translator, dictation, clipboard, notes and extensions, with Raycast prices and sources.',
  crumbs: [['Home', '/'], ['Ferrocast vs Raycast', '/ferrocast-vs-raycast/']],
  h1: 'Ferrocast vs Raycast: what is free, what is paid',
  lead: [
    '**Is Raycast free?** Yes, for its core: clipboard history, quicklinks, calculator, snippets, window management and thousands of extensions. Raycast AI, Cloud Sync, Translator, Dictation and unlimited notes and clipboard history are in the paid plans, from $10 a month or $96 a year ([raycast.com/pricing](https://www.raycast.com/pricing), checked 27 October 2026).',
    'Ferrocast is a free, open-source (MIT) launcher for Apple Silicon Macs with no paid plan. It covers several of the things Raycast charges for, with your own AI provider, and it lacks others, such as sync and an extension store. The table below shows both sides. I make Ferrocast, so check the sources.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Raycast Free vs Raycast Pro vs Ferrocast',
    caption: 'Features of Raycast Free, Raycast Pro and Ferrocast',
    cols: ['Feature', 'Raycast Free', 'Raycast Pro ($10/month)', 'Ferrocast (free)'],
    rows: [
      ['Price', '$0', '$10/month or $96/year; Pro+ $20, Max $50 a month', '$0, no paid plan'],
      ['Clipboard history', 'Yes, 3 months', 'Unlimited', 'Yes; you choose how long to keep it, and it skips password managers'],
      ['Snippets, quicklinks, calculator', 'Yes', 'Yes', 'Yes'],
      ['Window management', 'Yes', 'Yes, plus custom commands', 'About 40 built-in commands'],
      ['Notes', '5 notes', 'Unlimited', 'Floating Markdown notes'],
      ['AI chat', 'Not included', 'Included, with 500 monthly credits', 'Ask AI with your provider: Ollama on your Mac by default, or your own key'],
      ['Bring your own AI key', 'Not included', 'Included', 'Yes (OpenAI-compatible servers, Anthropic, Claude Code)'],
      ['AI with screen or selection context', 'Not included', 'Included (Screen Awareness)', 'No'],
      ['Translator', 'Not included', 'Included', 'Translate selected text with your AI provider'],
      ['Dictation', 'Not included', 'Included', 'In Notes (on-device recognition where macOS supports it)'],
      ['Extensions', 'Thousands in the Store; custom extensions', 'Same', 'Raycast-compatible Script Commands; no store, no React extensions'],
      ['Cloud Sync', 'Not included', 'Included', 'No'],
      ['Local file conversion', 'Not built in, per its website', 'Not built in, per its website', '275 conversions, 26 file tools'],
      ['Screen recording', 'Not stated on its pricing page', 'Not stated on its pricing page', 'Yes, with auto zoom'],
      ['Platforms', 'macOS, Windows, iOS', 'macOS, Windows, iOS', 'macOS, Apple Silicon only'],
      ['Account', 'Needed for the 7-day Pro trial and to subscribe', 'Required', 'No account'],
      ['Source code', 'No open licence stated', 'No open licence stated', 'MIT'],
    ],
    note: 'Raycast columns from [raycast.com/pricing](https://www.raycast.com/pricing) and [manual.raycast.com/billing](https://manual.raycast.com/billing), checked 27 October 2026. Ferrocast column from its [key facts](/facts/) and docs.',
  },
  steps: {
    h: 'Try Ferrocast next to Raycast',
    howToName: 'Try Ferrocast alongside Raycast on a Mac',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew. It needs no account.'],
      ['Pick a different hotkey', 'Ferrocast opens with ⌥ Space by default; change it in Settings if Raycast already uses it.'],
      ['Bring your Script Commands and snippets', 'Add your Script Commands folder in Settings → Scripts and import your snippets from a Raycast JSON export.'],
      ['Set up Ask AI', 'Install Ollama for a local model, or add your own API key. Keys are stored in the Keychain.'],
    ],
  },
  sections: [
    {
      h: 'Where Raycast is ahead',
      paras: [],
      list: [
        'Thousands of extensions in its Store, and React extensions you can write yourself.',
        'Cloud Sync between Macs, plus Windows and iOS apps under one subscription.',
        'Raycast AI with hosted models, monthly credits and context from your screen, files and memory (paid plans).',
        'Shared commands, snippets and quicklinks for teams.',
      ],
    },
    {
      h: 'Where Ferrocast is ahead',
      paras: [],
      list: [
        'No paid plan: AI chat, translation and dictation in Notes are free, with your own model or key.',
        'Local file work built in: [275 conversions](/files/) and [26 file tools](/tools/), nothing uploaded.',
        'Screen recording with auto zoom and an on-device transcript.',
        'Open source under MIT, no account.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is Raycast free?', 'Yes. Raycast Free includes clipboard history (3 months), quicklinks, calculator, snippets, window management, five notes and the extension Store. Raycast AI, Cloud Sync, Translator and Dictation need a paid plan, from $10 a month.'],
      ['What does Raycast Pro add?', 'Raycast AI with 500 monthly credits, bring your own key, Cloud Sync, Dictation, Translator, unlimited notes and clipboard history, custom window management commands and custom themes, for $10 a month or $96 a year.'],
      ['Is there a free alternative to Raycast Pro features?', 'For some of them. Ferrocast is free and includes AI chat with your own provider (Ollama by default), translation of selected text, dictation in notes and clipboard history. It has no sync and no extension store.'],
      ['Does Ferrocast run Raycast extensions?', 'No. Ferrocast runs Raycast Script Commands, not Raycast Store or React extensions.'],
      ['Does Ferrocast work on Windows?', 'No. Ferrocast is macOS only and needs an Apple Silicon Mac.'],
      ['Is Ferrocast affiliated with Raycast?', 'No. Ferrocast is independent and not affiliated with Raycast.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['Run Raycast Script Commands', '/raycast-script-commands/'],
      ['Migrating from Raycast', '/docs/getting-started/migrating-from-raycast/'],
      ['Local AI in the launcher', '/local-ai-launcher/'],
      ['Key facts', '/facts/'],
    ],
  },
};
