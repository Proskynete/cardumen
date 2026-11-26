# tabs Specification

## Purpose
TBD - created by archiving change tabs-and-splits. Update Purpose after archive.
## Requirements
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

### Requirement: Rename tab
Double-clicking a tab, or pressing Cmd+Shift+R on the active tab, SHALL turn its title into an inline text field with an accent caret, prefilled with the current title. While editing, keystrokes SHALL go to the field and not to the shell: Enter or a click elsewhere SHALL save, Escape SHALL cancel, Backspace SHALL delete a character and Cmd+Backspace SHALL clear the field. A saved name SHALL replace any automatic title (OSC or directory) until it is renamed again. Saving an empty name SHALL restore the automatic title.

#### Scenario: Name a tab
- **WHEN** the user double-clicks a tab, types "api" and presses Enter
- **THEN** the tab shows "api", even after Claude Code sets a window title

#### Scenario: Clear the name
- **WHEN** the user renames a tab to an empty string
- **THEN** the tab goes back to showing the directory or program title

#### Scenario: Cancel
- **WHEN** the user presses Escape while renaming
- **THEN** the tab keeps its previous title and the shell received no keystrokes

### Requirement: Reorder tabs
Dragging a tab horizontally by more than 4 points SHALL lift it: it follows the pointer (keeping the grab offset, clamped to the tab bar) and is drawn above the others. When its centre crosses into a neighbour's slot, they swap, and the other tabs slide to their new slots with an eased animation. On release, the dragged tab slides into its slot. The dragged tab SHALL stay active. Cmd+Shift+Left and Cmd+Shift+Right SHALL move the active tab one slot left or right (no wrap).

#### Scenario: Drag first tab to the end
- **WHEN** tabs A, B, C exist and the user drags A over C's slot and releases
- **THEN** the order is B, C, A and A is active

#### Scenario: Keyboard move
- **WHEN** tab B of A, B, C is active and the user presses Cmd+Shift+Left
- **THEN** the order is B, A, C

### Requirement: Close button
The active tab and the tab under the pointer SHALL show a `×` at their right edge. Clicking it SHALL close that tab and terminate every shell in it, following the same rules as closing the last pane (the tab to the left becomes active; closing the last tab quits).

#### Scenario: Close an inactive tab
- **WHEN** the pointer is over inactive tab C and the user clicks its `×`
- **THEN** C and all its panes close, and the active tab does not change

