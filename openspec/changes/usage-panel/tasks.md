## 1. Data

- [x] 1.1 `usage.rs`: line parser (assistant usage with message-id dedup, user turns, project from cwd) with tests
- [x] 1.2 Incremental cache (binary format, per-file offsets, shrink detection) with a round-trip test
- [x] 1.3 `summarize`: KPIs, deltas, per-day series, heatmap, agent time, concurrency, model and project tables, with tests
- [x] 1.4 Short-lived worker thread → `UserEvent::Usage(Summary)`

## 2. View

- [x] 2.1 Cmd+U / Esc, input isolation, range pills
- [x] 2.2 KPI cards with delta pill and sparkline
- [x] 2.3 Tokens per day and heatmap
- [x] 2.4 Models and projects tables

## 3. Verify

- [x] 3.1 Screenshot on real data; first-load and second-load time; memory after closing
