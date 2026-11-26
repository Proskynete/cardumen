## 1. Model

- [ ] 1.1 `session_file.rs`: serde model, `capture` from `Tabs`, `rebuild` to `Tabs` with fresh ids, clamping and `$HOME` fallback
- [ ] 1.2 `load`/`store`: atomic write (temp + rename), delete when empty, `None` on any error
- [ ] 1.3 Unit tests: round-trip, invalid JSON, missing directory, clamped indices, file store/load

## 2. Wiring

- [ ] 2.1 Restore in `resumed` (sidebar/panel, per-tab spawn, banner on first pane only), fall back to one tab
- [ ] 2.2 Debounced save in `about_to_wait` when the structure changes
- [ ] 2.3 Save in `exiting` (Cmd+Q, window close, last tab closed)
