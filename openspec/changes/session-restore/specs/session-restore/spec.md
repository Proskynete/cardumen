## ADDED Requirements

### Requirement: Save the session
The system SHALL save the window structure to `~/Library/Application Support/cardumen/session.json`: the tabs in order, each tab's custom name, each tab's split tree with directions, each pane's working directory, the active tab, the focused pane of each tab, and whether the sidebar and the Resumen panel are visible. It SHALL save within about one second after any of these change, and on quit (Cmd+Q or closing the window). It SHALL write the file atomically (a temporary file renamed over the old one). Shell processes, scrollback and window geometry SHALL NOT be saved.

#### Scenario: Save after a split
- **WHEN** the user splits a pane with Cmd+D
- **THEN** within about one second `session.json` describes the new split

#### Scenario: Save on quit
- **WHEN** the user `cd`s to `~/projects/cardumen` and presses Cmd+Q
- **THEN** `session.json` records `~/projects/cardumen` for that pane

### Requirement: Clear the session when the last tab closes
When the last tab closes and the application quits because nothing is left, the system SHALL delete `session.json`, so the next launch starts with one tab.

#### Scenario: Last shell exits
- **WHEN** one tab with one pane is open and its shell exits
- **THEN** Cardumen quits and `session.json` no longer exists

### Requirement: Restore the session on launch
On launch, when `session.json` is valid, the system SHALL recreate its tabs in order with their names, split trees, active tab, focused panes, and sidebar and panel visibility. Each pane SHALL start a new login shell in its saved directory, or in `$HOME` when that directory no longer exists. Only the first pane of the first tab SHALL show the welcome banner.

#### Scenario: Two tabs with a split
- **WHEN** Cardumen quit with tab "api" (two panes side by side, the right one focused) and tab "web" active, and the sidebar hidden
- **THEN** on launch tab "api" has two side-by-side panes with the right one focused, tab "web" is active, and the sidebar is hidden

#### Scenario: Directory removed
- **WHEN** a saved pane's directory was deleted before launch
- **THEN** that pane's shell starts in `$HOME`

#### Scenario: Banner once
- **WHEN** a session with three panes is restored
- **THEN** only the first tab's first pane shows the welcome banner

### Requirement: Fall back on a bad session file
When `session.json` is missing, unreadable, not valid JSON, of an unknown version, or has no tabs, the system SHALL start with one tab as if no session existed, without crashing. Out-of-range active-tab or focused-pane indices SHALL be clamped to the last valid one.

#### Scenario: Corrupt file
- **WHEN** `session.json` contains invalid JSON
- **THEN** Cardumen opens one tab in `$HOME`

#### Scenario: Focus index out of range
- **WHEN** a saved tab has two panes and its focused index is 5
- **THEN** the second pane of that tab is focused
