/*
 * Copy for /local-ai-launcher/ (English, board idea "Mac launcher with local AI: Ollama, LM Studio,
 * Claude Code"). Ferrocast facts come from docs/launcher/ask-ai.md, translate.md and notes.md; the
 * Raycast line comes from raycast.com/pricing (AI and bring-your-own-key are in paid plans), read on
 * 2026-10-27. Rules: say plainly which providers send text off the Mac, keep the ChatGPT sign-in
 * caveat, and never claim AI context from the screen or selection.
 */
import type { GuideCopy } from './compress-video';

export const page: GuideCopy = {
  lang: 'en',
  path: '/local-ai-launcher/',
  title: 'Mac launcher with local AI: Ollama, LM Studio, Claude Code | Ferrocast',
  description:
    'Ferrocast is a free Mac launcher whose AI chat talks to Ollama on your Mac by default, or to LM Studio, your own API key or Claude Code. No subscription.',
  crumbs: [['Home', '/'], ['Local AI launcher', '/local-ai-launcher/']],
  h1: 'A Mac launcher with local AI, and your own keys when you want them',
  lead: [
    'Ask AI in Ferrocast is a streaming chat with conversation history. Out of the box it talks to **Ollama on your own Mac**, so nothing leaves it. You can switch to LM Studio, any OpenAI-compatible server, the Anthropic API with your key, or Claude Code with your Claude subscription, from a menu in the chat.',
    'Ferrocast is free and open source (MIT); there is no AI plan to buy. In Raycast, AI and bringing your own key are part of the paid plans ([raycast.com/pricing](https://www.raycast.com/pricing), checked 27 October 2026).',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Providers, and where your text goes',
    caption: 'AI providers supported by Ferrocast and whether the text leaves the Mac',
    cols: ['Provider', 'How Ferrocast uses it', 'Leaves your Mac?'],
    rows: [
      ['Ollama (default)', 'An OpenAI-compatible server running on your Mac; preset in Settings → AI.', 'No'],
      ['LM Studio', 'Same, with its own preset.', 'No'],
      ['OpenAI and other OpenAI-compatible servers', 'Your API key, stored in the Keychain; models listed from the server.', 'Yes, to that provider'],
      ['Anthropic API', 'Your own key, stored in the Keychain.', 'Yes, to Anthropic'],
      ['Claude Code', 'The official CLI with your Claude subscription, tools off, one warm session while the chat is open.', 'Yes, to Anthropic'],
      ['Sign in with ChatGPT', 'The OAuth flow Codex CLI uses, so a ChatGPT plan answers without an API key. OpenAI has not approved third-party use of it.', 'Yes, to OpenAI'],
    ],
    note: 'From the [Ask AI docs](/docs/launcher/ask-ai/). **Test connection** in Settings → AI asks the server for its model list.',
  },
  steps: {
    h: 'Set up local AI in a few minutes',
    howToName: 'Use a local AI model from the Ferrocast launcher with Ollama',
    items: [
      ['Install Ollama', 'Install Ollama from ollama.com and download a model with ollama pull followed by the model name.'],
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew.'],
      ['Check the connection', 'In Settings → AI, keep the Ollama preset and press Test connection; pick a model from the list.'],
      ['Ask', 'Open Ask AI from the launcher and type. Switch provider or model from the chip in the chat.'],
    ],
  },
  sections: [
    {
      h: 'Where else the AI is used',
      list: [
        '**Translate selected text:** drag a selection, tap ⌘ and drop it on a language; the translation replaces the text where it can. See [Translate](/docs/launcher/translate/).',
        '**Notes:** AI commands in the floating notes window. See [Notes](/docs/launcher/notes/).',
        'Each feature can use its own provider and model.',
      ],
    },
    {
      h: 'Limits',
      list: [
        'Ask AI does not read your screen or the current selection; you paste what you want to ask about.',
        'Sign in with ChatGPT is a grey area of OpenAI’s terms; an API key is the by-the-book route.',
        'ChatGPT and Claude subscriptions do not include API access, which is why they are offered through sign-in and Claude Code.',
        'Local models need the memory and disk space Ollama or LM Studio asks for.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Is there a Mac launcher that works with Ollama?', 'Yes. Ferrocast’s Ask AI talks to Ollama on your Mac by default, through its OpenAI-compatible server, and keeps the chat on your Mac.'],
      ['Can I use LM Studio instead of Ollama?', 'Yes. Settings → AI has an LM Studio preset; Ferrocast lists the models the server reports.'],
      ['Can I use my Claude subscription?', 'Yes, through Claude Code, the official CLI, if it is installed. Ferrocast runs it with tools off and keeps one session warm while the chat is open.'],
      ['Do I need a subscription for AI in Ferrocast?', 'No. Ferrocast has no paid plan. A local model is free; cloud providers bill you directly for your own key.'],
      ['Are my API keys safe?', 'Keys are stored in the macOS Keychain, not in Ferrocast’s settings files.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Ask AI docs', '/docs/launcher/ask-ai/'],
      ['En español: IA local en Mac', '/es/ia-local-mac/'],
      ['Translate selected text', '/docs/launcher/translate/'],
      ['Guide: translate selected text on a Mac', '/translate-selected-text-mac/'],
      ['Ferrocast vs Raycast: free vs Pro', '/ferrocast-vs-raycast/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
    ],
  },
};
