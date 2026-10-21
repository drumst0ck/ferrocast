---
title: "Currency and time zones"
description: "Convert currencies with ECB rates cached for 12 hours and time zones such as time in tokyo or 5pm ldn in sf directly from the Ferrocast search field."
status: tested
sidebar:
  order: 3
---
Type a currency conversion or a time-zone query in the root search and the answer appears inline. Currency uses ECB rates through frankfurter.dev with a 12-hour cache; rates refresh when they are stale.

Examples: `100 usd in eur`, `time in tokyo`, `5pm ldn in sf`.

## Options

- `est`, `pst` and `cet` are fixed offsets.
- Clocks inside a daylight-saving gap still resolve.
- Small amounts work, for example `1 jpy in usd`.

:::caution[Limits]
Currency conversion requires network access.
:::

## Status

**Tested.** Implemented; currency needs a network connection.

## Related

- [Calculator](/docs/launcher/calculator/)
- [Root search](/docs/launcher/search/)
