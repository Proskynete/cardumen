## 1. Status

- [x] 1.1 `status.rs`: line parser and state machine (unit tests); socket listener thread → `UserEvent::Status`
- [x] 1.2 Pane env: `CARDUMEN_SOCKET`, `CARDUMEN_PANE`; remove the socket on exit
- [x] 1.3 `scripts/install-hooks.py`: backup + idempotent merge; run it

## 2. Sidebar

- [x] 2.1 Layout: sidebar width reserved when visible; Cmd+B toggles
- [x] 2.2 Rows grouped by git project, status dot, elapsed time, focused highlight, summary line
- [x] 2.3 Click a row to jump; warm dot on tabs with waiting panes; Dock bounce when waiting in background

## 3. Verify

- [x] 3.1 End-to-end test: a fake hook line over the socket changes a pane's state
- [x] 3.2 Screenshot with two projects and different states
