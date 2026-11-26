## ADDED Requirements

### Requirement: Split
Cmd+D SHALL split the focused pane into two side by side, and Cmd+Shift+D SHALL split it into two stacked. The new pane SHALL run a new login shell in the focused pane's working directory, without the welcome banner, and SHALL take focus. Splits SHALL nest, and siblings SHALL share space evenly.

#### Scenario: Split right
- **WHEN** a tab has one pane and the user presses Cmd+D
- **THEN** the tab shows two panes of equal width separated by a 1-point divider, and the right one has focus

#### Scenario: Nested split
- **WHEN** the user presses Cmd+D and then Cmd+Shift+D
- **THEN** the left pane keeps its full height and the right half is split into top and bottom

### Requirement: Pane size
Each pane SHALL have its own grid size computed from its rectangle, and its PTY SHALL be resized whenever the layout or window changes.

#### Scenario: tput in a split
- **WHEN** a 120-column window is split right
- **THEN** `tput cols` in each pane reports about half of the columns

### Requirement: Focus
Exactly one pane per tab SHALL have focus. A click inside a pane SHALL focus it. Cmd+Option+Arrow SHALL move focus to the nearest pane in that direction. Keyboard input, paste and scroll SHALL go to the focused pane. Unfocused panes SHALL draw a hollow cursor and SHALL NOT blink.

#### Scenario: Move focus left
- **WHEN** two side-by-side panes exist, the right one is focused, and the user presses Cmd+Option+Left
- **THEN** the left pane gets focus

### Requirement: Close pane
Cmd+W SHALL close the focused pane (terminating its shell). When a pane's shell exits, the pane SHALL close. Its sibling SHALL take over the freed space and focus.

#### Scenario: exit in a split
- **WHEN** two panes exist and the user types `exit` in the right one
- **THEN** the left pane fills the tab and has focus
