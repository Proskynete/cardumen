# session-sidebar Specification

## Purpose
TBD - created by archiving change session-sidebar. Update Purpose after archive.
## Requirements
### Requirement: Sidebar
A 230-point sidebar SHALL be shown at the left by default and toggled with Cmd+B. Its top band holds the window controls. Below, two summary lines SHALL show how many sessions are waiting and how many are working (`● 1 te esperan`, `● 3 trabajando`). Panes SHALL be grouped under their project name (the name of the git repository containing the pane's directory, else the directory name). Each row SHALL show a status dot and the tab's title, and under it the state with elapsed time: "te espera · 3m" (warm), "trabajando · 21m" (accent), "lista · 1m" (success), or the program title for panes without Claude Code (muted). The focused pane's row SHALL be highlighted. The tab bar and panes SHALL start to the right of the sidebar.

#### Scenario: Two projects
- **WHEN** panes run in `~/projects/cardumen` and `~/projects/blog/terminal` (a git repo named `terminal`)
- **THEN** the sidebar shows groups "cardumen" and "terminal" with their rows

#### Scenario: Toggle
- **WHEN** the user presses Cmd+B
- **THEN** the sidebar hides and the panes take the full width

### Requirement: Jump to a session
Clicking a row SHALL activate its tab and focus its pane.

#### Scenario: Jump
- **WHEN** the user clicks a row for a pane in tab 3
- **THEN** tab 3 becomes active with that pane focused

### Requirement: Attention signals
A tab with any waiting pane SHALL show a warm dot before its title. When a pane becomes waiting while the window is not focused, the Dock icon SHALL bounce once.

#### Scenario: Background permission request
- **WHEN** Cardumen is in the background and Claude Code in some pane asks for permission
- **THEN** the Dock icon bounces and that tab shows a warm dot

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

