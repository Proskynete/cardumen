## MODIFIED Requirements

### Requirement: Status socket
On startup Cardumen SHALL listen on a Unix socket at `$TMPDIR/cardumen-<pid>.sock` with mode `0600`, remove it on exit, and start every shell with `CARDUMEN_SOCKET` (that path) and `CARDUMEN_PANE` (the pane id). Each line received SHALL be `<pane id> <event>`; malformed lines and unknown panes SHALL be ignored.

#### Scenario: Event from a hook
- **WHEN** a process in pane 3 writes `3 waiting` to the socket
- **THEN** pane 3's Claude state becomes "waiting"

#### Scenario: Owner-only socket
- **WHEN** Cardumen starts
- **THEN** the socket file's mode is `0600`
