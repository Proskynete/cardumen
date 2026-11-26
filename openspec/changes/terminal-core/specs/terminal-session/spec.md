## ADDED Requirements

### Requirement: Login shell in a PTY
The system SHALL start the user's login shell (`$SHELL`, falling back to `/bin/zsh`) as a login shell inside a PTY, in the user's home directory, with `TERM=xterm-256color` and `COLORTERM=truecolor`.

#### Scenario: zsh config loads
- **WHEN** the user opens Cardumen
- **THEN** the shell runs with `-l`, and the prompt and aliases defined in `~/.zshrc` are available

#### Scenario: SHELL unset
- **WHEN** `$SHELL` is empty
- **THEN** the system starts `/bin/zsh -l`

### Requirement: VT emulation
The system SHALL interpret shell output with a full VT/xterm emulator, including the alternate screen, 256-colour and truecolor SGR, cursor styles, bracketed paste, and mouse reporting.

#### Scenario: Full-screen TUI
- **WHEN** the user runs `claude` or `vim`
- **THEN** the program renders on the alternate screen, and quitting restores the previous screen

### Requirement: Keyboard input
The system SHALL send key presses to the PTY using xterm encodings, including Ctrl combinations, arrows, Home/End, PageUp/PageDown, function keys, and Option as Meta (ESC prefix).

#### Scenario: Interrupt
- **WHEN** the user presses Ctrl+C
- **THEN** byte `0x03` is written to the PTY

#### Scenario: Option as Meta
- **WHEN** the user presses Option+B
- **THEN** the bytes `ESC b` are written to the PTY

### Requirement: Clipboard
The system SHALL copy the current selection on Cmd+C and paste clipboard text on Cmd+V, wrapped in bracketed-paste markers when the application enabled that mode.

#### Scenario: Paste into a bracketed-paste app
- **WHEN** Claude Code is running and the user presses Cmd+V
- **THEN** the pasted text is sent between `ESC[200~` and `ESC[201~`

### Requirement: Mouse selection
The system SHALL let the user select text with a left-drag, select a word with a double-click and a line with a triple-click, unless the application requested mouse reporting, in which case mouse events go to the application (Shift overrides).

#### Scenario: Drag to select
- **WHEN** the user drags across text in the shell
- **THEN** the cells are highlighted, and Cmd+C copies exactly that text

#### Scenario: Copy on select
- **WHEN** the user releases the mouse after selecting text (drag, double-click or triple-click)
- **THEN** the selected text is placed on the clipboard without pressing Cmd+C

### Requirement: Scrollback
The system SHALL keep at least 10,000 lines of scrollback (configurable) and scroll it with the trackpad or mouse wheel on the primary screen. On the alternate screen, the wheel SHALL be sent to the application as arrow keys or mouse events.

#### Scenario: Scroll history
- **WHEN** output exceeds the window height and the user scrolls up
- **THEN** older lines are shown, and typing returns the view to the bottom

### Requirement: Resize
The system SHALL recompute columns and rows when the window is resized or its scale factor changes, and notify the PTY (`TIOCSWINSZ`).

#### Scenario: Resize window
- **WHEN** the user resizes the window
- **THEN** `tput cols` reports the new width

### Requirement: Shell exit
The system SHALL close the window when the shell process exits.

#### Scenario: exit
- **WHEN** the user types `exit`
- **THEN** the window closes and the process terminates with code 0
