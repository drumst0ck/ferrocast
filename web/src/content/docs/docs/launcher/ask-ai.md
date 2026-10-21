---
title: "Ask AI"
description: "Chat with a model from Ferrocast using Ollama, any OpenAI-compatible server, ChatGPT sign-in, the Anthropic API or Claude Code. Keys stay in the Keychain."
status: tested
sidebar:
  order: 11
---
Ask AI is a streaming chat with conversation history. It supports four providers: an OpenAI-compatible server (Ollama by default, LM Studio, OpenAI and others), **Sign in with ChatGPT**, the Anthropic API (your own key) and **Claude Code** (the official CLI, with your Claude subscription). Keys are stored in the Keychain.

Open Ask AI from the launcher. The provider chip is a menu to switch between the providers that are ready right now (configured server, ChatGPT signed in, Anthropic key saved, Claude Code installed) and between that provider's models, without leaving the chat; the choice is saved. Out of the box it talks to Ollama on your own Mac, so nothing leaves it unless you point it elsewhere.

## Options (Settings → AI)

- Provider presets (Ollama, LM Studio, OpenAI), Keychain status for the key and a **Test connection** button that asks the server for its model list.
- The model is a dropdown fed by each provider (the server's `/models` list for OpenAI-compatible and Anthropic, the ChatGPT backend catalogue, Claude Code's aliases), with Default and Custom… entries and a refresh button.
- **Sign in with ChatGPT** uses the same OAuth flow and backend as OpenAI's Codex CLI, so a ChatGPT plan answers without an API key. The session is kept in the Keychain and refreshed automatically.
- **Claude Code** runs with tools off, the prompt on stdin and its own system prompt. Ferrocast keeps one warm CLI session while the view is open, started in the background when you open Ask AI: measured 3.2 s for a cold first answer versus 1.4 s for every following turn, with the conversation context kept by the CLI.

ChatGPT and Claude subscriptions do not include API access, so they are not offered directly.

:::caution[Limits]
Sign in with ChatGPT is a grey area of terms: OpenAI has not blessed third-party use of that flow, and the Settings pane says so plainly; the API with a key is the by-the-book route.
:::

## Status

**Tested.** Implemented for all four providers.

## Related

- [Settings](/docs/launcher/settings/)
- [Data and privacy](/docs/reference/data-and-privacy/)
