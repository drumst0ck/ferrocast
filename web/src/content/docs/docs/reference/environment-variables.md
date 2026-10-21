---
title: "Environment variables"
description: "Environment variables read by Ferrocast and its helper scripts: FERROCAST_DATA_DIR, FERROCAST_QUERY, FERROCAST_PROFILE, FERROCAST_VIEW and FERROCAST_PANEL."
sidebar:
  order: 2
---
These variables change where Ferrocast stores data and drive the screenshot and performance scripts. They are mainly for development.

| Variable | Effect |
|---|---|
| `FERROCAST_DATA_DIR` | Changes the data folder from `~/Library/Application Support/Ferrocast`. Use a throwaway folder so real data is never touched. |
| `FERROCAST_QUERY` | Search text typed for `scripts/shot.sh`, for example `FERROCAST_QUERY="100 usd in eur"`. |
| `FERROCAST_PROFILE` | `1` makes `scripts/perf.sh` print slow frames. |
| `FERROCAST_VIEW` | View rendered by `scripts/shot.sh`: clipboard, snippets, quicklinks, emoji, settings, notes, ai, welcome, shortcuts, about. |
| `FERROCAST_PANEL` | `1` renders the ⌘K panel in `scripts/shot.sh`. |

Example:

```bash
scripts/shot.sh out.png FERROCAST_QUERY="100 usd in eur"
scripts/shot.sh out.png FERROCAST_VIEW=settings
```

`scripts/shot.sh` renders the UI to a PNG from inside egui and needs no screen permission.

## Related

- [Contributing](/docs/contributing/)
- [Data and privacy](/docs/reference/data-and-privacy/)
