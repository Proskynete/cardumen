## ADDED Requirements

### Requirement: Status socket
On startup Cardumen SHALL listen on a Unix socket at `$TMPDIR/cardumen-<pid>.sock`, remove it on exit, and start every shell with `CARDUMEN_SOCKET` (that path) and `CARDUMEN_PANE` (the pane id). Each line received SHALL be `<pane id> <event>`; malformed lines and unknown panes SHALL be ignored.

#### Scenario: Event from a hook
- **WHEN** a process in pane 3 writes `3 waiting` to the socket
- **THEN** pane 3's Claude state becomes "waiting"

### Requirement: State machine
Events SHALL map to states: `prompt` and `tool` → working, `waiting` → waiting, `stop` → done, `end` → no Claude session. Each change SHALL record the time it happened.

#### Scenario: Permission then approval
- **WHEN** pane 1 receives `prompt`, `waiting`, then `tool`
- **THEN** its state goes working → waiting → working

### Requirement: Claude Code hooks
`scripts/install-hooks.py` SHALL back up `~/.claude/settings.json` and add, without removing existing hooks and without duplicating its own entries: `UserPromptSubmit` → `prompt`, `PostToolUse` → `tool`, `PermissionRequest` → `waiting`, `Stop` and `StopFailure` → `stop`, `SessionEnd` → `end`. Each hook SHALL do nothing and exit 0 when `CARDUMEN_SOCKET` is unset, and SHALL never print to stdout.

#### Scenario: Claude outside Cardumen
- **WHEN** Claude Code runs in another terminal
- **THEN** the hooks exit immediately and nothing changes

#### Scenario: Run twice
- **WHEN** the installer runs a second time
- **THEN** settings.json is unchanged
