---
title: "Calculator, units and colours"
description: "Type math, percentages, units and colour values into the Ferrocast search field and get the answer inline, with a history of your last 100 calculations."
sidebar:
  order: 2
---
The calculator evaluates what you type in the root search: math, percentages, unit conversion, currency and colour conversion. URLs typed in the field are detected as URLs.

Type the expression in the launcher search field, for example `200 * 10%`, `pi/2`, `2 x 3`, `1,000 * 2` or `1 jpy in usd`. Currency and time-zone queries are covered in [Currency and time zones](/docs/launcher/currency-and-time-zones/).

## Pick a colour from the screen

Run **Pick Color** from the launcher. The launcher hides and the macOS colour sampler appears: move the magnifier over any pixel and click to pick it, or press <kbd>Esc</kbd> to cancel. The colour is copied as HEX (for example `#FF8000`) and also lands in the clipboard history. Paste it back into the search field to see it as RGB, HSL and the other formats.

## History

The **Calculator History** command keeps the last 100 results; delete one entry or clear them all.

## Fixed in the current changelog

`200 * 10%`, `pi/2`, `2 x 3`, `1,000 * 2` and small currency amounts such as `1 jpy in usd` now evaluate correctly.

:::caution[Limits]
Currency conversion needs network access (see [Currency and time zones](/docs/launcher/currency-and-time-zones/)).
:::

## Related

- [Currency and time zones](/docs/launcher/currency-and-time-zones/)
- [Root search](/docs/launcher/search/)
