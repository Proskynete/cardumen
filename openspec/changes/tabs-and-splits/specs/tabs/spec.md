## ADDED Requirements

### Requirement: Tab bar
The system SHALL draw a tab bar in the titlebar band, to the right of the window controls. Each tab SHALL show its title, truncated with `…` when it does not fit. The active tab SHALL have a lighter background, primary-colour text and a 2-point accent line under it. Inactive tabs SHALL use muted text. A `+` button SHALL follow the last tab.

#### Scenario: Two tabs
- **WHEN** two tabs are open and the second is active
- **THEN** the second tab is drawn with the accent line and the first in muted text

### Requirement: Tab title
A tab's title SHALL be the title set by the focused pane's program (OSC 0/2). When no title is set, or the title is the shell prompt's `user@host…` form (it starts with `$USER@`), it SHALL be the basename of the focused pane's working directory (`~` for the home directory).

#### Scenario: Claude Code sets a title
- **WHEN** Claude Code sets the window title to "✳ Refactor auth"
- **THEN** the tab shows "✳ Refactor auth"

#### Scenario: Prompt title
- **WHEN** zsh sets the title to "proskynete@Mini: ~/projects/cardumen"
- **THEN** the tab shows "cardumen"

### Requirement: New tab
Cmd+T and a click on `+` SHALL open a new tab after the active one, running a new login shell in the focused pane's working directory, show the welcome banner, and make it active.

#### Scenario: Inherit directory
- **WHEN** the focused pane is in `~/projects/cardumen` and the user presses Cmd+T
- **THEN** the new tab's shell starts in `~/projects/cardumen`

### Requirement: Select tab
Cmd+1…Cmd+8 SHALL select tabs 1–8, Cmd+9 the last tab, Cmd+Shift+] the next and Cmd+Shift+[ the previous (wrapping around). A click on a tab SHALL select it.

#### Scenario: Cmd+9
- **WHEN** 5 tabs are open and the user presses Cmd+9
- **THEN** tab 5 becomes active

### Requirement: Close tab
When the last pane of a tab closes (Cmd+W or shell exit), the tab SHALL close and the tab to its left (or the new first tab) SHALL become active. When the last tab closes, the application SHALL quit.

#### Scenario: Close middle tab
- **WHEN** tabs A, B, C are open, B is active and its only shell exits
- **THEN** B is removed and A becomes active
