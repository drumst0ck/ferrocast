## Ferrocast perf baseline — 0d660f5 — Apple M3 — 27.0

| Metric | Result |
|---|---|
| hidden start: window+gl ready, median of 5 | 121.1 ms |
| hidden start: App::new, median | 10.6 ms |
| hidden start: apps scan, median (153 apps on the dev Mac) | 125.8 ms |
| hidden idle (15s after start, 10s window): CPU | 0.55 % |
| hidden idle: context switches/s | 241 |
| hidden idle: frames drawn while hidden (sum of printed 10 s windows: 2; none printed = 0) | 243 |
| hidden idle: slow frames while hidden | 3 |
| physical footprint after 15s hidden / after idle window | 159.1M / 163.4M |
| view: root (empty query) | sections max n/a ms; icon sync 8 ms (3 renders); slow frames 2 |
| view: root query 'saf' | sections max n/a ms; icon sync 6 ms (3 renders); slow frames 2 |
| view: clipboard, 1000 text + 50 images | sections max 2.31775 ms; icon sync 9 ms (4 renders); slow frames 3 |
| view: clipboard, query 'line 5' | sections max 1.41529 ms; icon sync 6 ms (4 renders); slow frames 3 |
| view: emoji, empty | sections max 0.931625 ms; icon sync 43 ms (12 renders); slow frames 7 |
| view: emoji, query 'smile' | sections max 0.488042 ms; icon sync 23 ms (9 renders); slow frames 6 |
| view: settings | sections max n/a ms; icon sync 5 ms (3 renders); slow frames 3 |
