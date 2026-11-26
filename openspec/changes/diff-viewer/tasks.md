## 1. Data

- [x] 1.1 `context::diff_base()` shared with `changes()`
- [x] 1.2 `diff.rs`: capped git runner, unified-diff parser (renames, binary, new/deleted, no-newline, per-file cap) with tests
- [x] 1.3 Untracked files as added (binary detection, byte budget) with a test
- [x] 1.4 Short-lived worker → `UserEvent::Diff`

## 2. View

- [x] 2.1 Overlay over the pane area: header, file list, diff rows (virtualised), truncation notice
- [x] 2.2 Scroll math (`visible`, `clamp`, `reveal`) with tests
- [x] 2.3 Open from Cambios and Cmd+G; Esc/Cmd+G close; input isolation; keys, wheel and clicks

## 3. Verify

- [x] 3.1 Release build without warnings, all tests pass, screenshot on this repo
