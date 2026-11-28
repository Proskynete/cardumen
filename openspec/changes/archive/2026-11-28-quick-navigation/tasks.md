## 1. Cmd+J

- [x] 1.1 Pure helper: next waiting pane given panes with `since` and the focused pane (oldest first, wrap), with tests
- [x] 1.2 Bind Cmd+J; reuse the sidebar-click jump to activate the tab and focus the pane

## 2. Fuzzy matching

- [x] 2.1 `fuzzy::score(query, text) -> Option<score>`: in-order subsequence, case/accent-insensitive, bonus for runs and word starts
- [x] 2.2 Tests: prefix beats scattered, accents (`contab` matches "Contabilidád"), no match, empty query

## 3. Cmd+P switcher

- [x] 3.1 State: query, selection, filtered tab list (title + project from the sidebar's grouping)
- [x] 3.2 Draw the box like the find bar: field, up to 8 rows, warm dot for waiting tabs, "Sin resultados"
- [x] 3.3 Keys: typing, Backspace, ↑/↓, Enter (activate + close), Esc and click outside (close); keys don't reach the pane
- [x] 3.4 Don't open over the diff or usage views (same rule as Cmd+F)

## 4. Docs and verify

- [x] 4.1 README shortcut table: Cmd+J and Cmd+P
- [ ] 4.2 `cargo test`, `cargo clippy`; check by hand with 10+ tabs and two waiting sessions
