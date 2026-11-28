## ADDED Requirements

### Requirement: Jump to a waiting session
Cmd+J SHALL activate the tab and focus the pane of a session waiting on the user, taking waiting panes from longest-waiting to newest and moving to the next one after the focused pane on each press, wrapping around. With no waiting session, Cmd+J SHALL do nothing.

#### Scenario: One waiting session
- **WHEN** only the pane in tab 4 is waiting and the user presses Cmd+J
- **THEN** tab 4 becomes active with that pane focused

#### Scenario: Cycle through several
- **WHEN** panes A (waiting 5m) and B (waiting 1m) are waiting and the user presses Cmd+J twice
- **THEN** focus goes to A, then to B

#### Scenario: Nothing waiting
- **WHEN** no session is waiting and the user presses Cmd+J
- **THEN** nothing changes
