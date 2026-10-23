---
title: "Translate selected text"
description: "Drag selected text, tap Command and drop it on a language: Ferrocast translates it with your AI and replaces it in place in text fields and web pages."
sidebar:
  order: 11
---
Select some text in any app, start dragging it, tap <kbd>⌘</kbd> and drop it on a language. Ferrocast sends the text to your AI provider and puts the translation back where the text was. Where that is not possible, it shows the translation in a window and copies it.

## Translate in three steps

1. Select the text and start dragging it. Keep the mouse button down.
2. Tap the translate key, <kbd>⌘</kbd> by default. A purple wheel with your languages opens under the pointer. <kbd>⇧</kbd> is not used because it extends the selection while you drag.
3. Release over a language. A glow shows the text being translated, and a moment later the translation replaces it.

Escape cancels. <kbd>⌘</kbd> and <kbd>⌃</kbd> do nothing while this wheel is open: it has no other wheel to switch to.

## Where the translation goes

Settings → Translate → **After translating** decides what happens with the result.

| Option | What it does |
|---|---|
| **Auto** (default) | Replaces the selection where the app allows it, and shows the window otherwise. |
| **Window** | Always shows the translation in a Ferrocast window, with Copy and “Paste into app”. |
| **Paste ⌘V** | Copies the translation and presses <kbd>⌘</kbd> <kbd>V</kbd> in the app. Only useful in editable fields. |
| **Copy only** | Puts the translation on the clipboard. |

With **Auto**, Ferrocast checks the app the text came from:

- **Text fields** (Notes, Mail, TextEdit, most native apps): the selection is replaced through Accessibility. If the app refuses, Ferrocast pastes with <kbd>⌘</kbd> <kbd>V</kbd>.
- **Web pages in Safari and Chromium browsers** (Chrome, Brave, Edge, Arc, Vivaldi, Opera): the selected text is replaced inside the page. This needs one browser setting, see below. The change lasts until you reload the page.
- **Anything else**, or any failure: the window opens with the translation, which is also on the clipboard.

If you switch to another app while the translation runs, Ferrocast never writes into that app. It shows the window instead.

## Replace text in web pages

The browser has to let Ferrocast run a small script in the page. Turn this on once per browser profile:

- **Chrome, Brave, Edge and other Chromium browsers**: View → Developer → **Allow JavaScript from Apple Events**. In Brave in Spanish the menu is Ver → Opciones para desarrolladores → Permitir JavaScript desde Eventos de Apple.
- **Safari**: turn on Settings → Advanced → Show features for web developers, then **Allow JavaScript from Apple Events** in Settings → Developer (in older Safari versions it is in the Develop menu).

The first time, macOS asks whether Ferrocast may control the browser: allow it. You can change that later in System Settings → Privacy & Security → Automation.

If the setting is off, the window opens with the translation and a message tells you which option to turn on.

## The translating effect

While the AI answers, the text being translated glows in Ferrocast's neon red:

- in Safari and Chromium pages, a neon outline with lights running around it surrounds the selected block, and the page's own selection colour is hidden until the translation arrives;
- in other apps that report where the selection is, a glowing outline is drawn around it;
- otherwise, a small “Translating…” pill appears where you dropped.

If the translation fails, the page is left exactly as it was and your selection comes back. With Reduce Motion on, the glow is static. Settings → Translate → **Translating effect** turns it off.

## Engine and model

Translation uses the same providers as [Ask AI](/docs/launcher/ask-ai/): an OpenAI-compatible server (Ollama keeps everything on your Mac), the Anthropic API, Sign in with ChatGPT or Claude Code (your ChatGPT or Claude subscription).

In Settings → Translate → **Engine**:

- **Same as chat** (default) uses the provider and model of Ask AI.
- A fixed provider lets you pick its **Model**: the provider's list, **Provider default**, or **Custom…** to type a name. Switching provider resets the model.
- **Apple** translates on your Mac with Apple Translation, with no AI provider, key or server. The text is not sent anywhere. It needs macOS 26 or later; on older versions the segment is not shown. See [Translate offline with Apple](#translate-offline-with-apple).

The server address, keys and sign-ins are set in Settings → AI; the Translate tab only chooses who translates. The selected text is sent as data, separate from the instructions, so a single word, a heading, a question or a sentence that reads like an order ("Summarize this", "Ignore the above") is translated instead of answered or followed. Your Mac then checks the language of the answer on-device, and if it came out in another language (for example when the text says "translate this into German instead"), Ferrocast asks once more.

## Translate offline with Apple

Apple Translation is part of macOS. It translates on your Mac, so the selected text never leaves it and there is no account, key or server to set up.

1. Open Settings → Translate → **Engine** and choose **Apple**.
2. The **Apple languages** card lists every language on your wheel with the status Apple reports: **Installed**, **Needs download** or **Not supported by Apple**.
3. Drag text and drop it on a language as usual. The first translation after launch can take a few seconds while the model loads.

Whether a language must be downloaded first depends on your macOS version, and Apple decides it, not Ferrocast. On a Mac where Apple translates with a model that is already installed, every supported language reads **Installed** and nothing needs downloading. When Apple says a language **Needs download**, the card shows a **Download languages** row: press **Open Language settings**, download it in System Settings → General → Language & Region → **Translation Languages**, then press **Check again**. If you drop text on a language that Apple says is not downloaded, Ferrocast says so and tells you where to get it; if Apple does not support the language, it suggests another language or an AI engine.

Each status is one language pair: the language with your Mac's system language, or with English for the system language itself. Translating from another language can need that language too.

On macOS older than 26 a stored Apple choice falls back to the chat provider, with a notice the first time. Apple Translation does not follow instructions, and the source language is detected on your Mac.

## Languages

Settings → Translate → **Languages** is a comma-separated list of up to 12 languages, in the order they appear on the wheel. The default is English, Spanish, French, German, Portuguese, Italian, Japanese and Chinese. Duplicates are removed and an empty list brings the defaults back. The source language is detected by the model, or on your Mac with the Apple engine.

## Privacy

The selected text is sent only to the provider you chose, and with Ollama or the Apple engine it never leaves your Mac. Ferrocast keeps neither the text nor the translation: the translation is only put on the clipboard, and Ferrocast's own clipboard history skips it. The log records the language, the provider, the length of the text and how long it took, never the text.

:::caution[Limits]
- Firefox is not supported for in-page replacement.
- Browser pages such as `brave://` or `chrome://` and the extension stores do not accept scripts, and text inside frames (some editors and embedded comments) is not replaced: the window opens instead.
- Pages that redraw their text (some web apps) can undo the replacement.
- Selections over 20,000 characters are refused.
:::

## When it does not work

`~/Library/Application Support/Ferrocast/ferrocast.log` has one line per translation (`translate Spanish via Claude Code ok chars=… ms=…`) and one per delivery, for example `translate deliver Spanish -> web-replace Chromium("Brave Browser")` followed by `ok`, or `fallback-show: js-permission` when the browser setting is off.

With the Apple engine the log line reads `translate Spanish via Apple ok chars=… ms=…`. Failures show a toast: "… is not downloaded for Apple Translation" means the language still has to be downloaded in System Settings, and "Apple Translation does not support …" means Apple has no model for that language.

## Related

- [Ask AI](/docs/launcher/ask-ai/)
- [Settings](/docs/launcher/settings/)
- [The drag wheel](/docs/files/drag-wheel/)
