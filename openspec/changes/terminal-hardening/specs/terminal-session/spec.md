## MODIFIED Requirements

### Requirement: Clipboard
The system SHALL copy the current selection on Cmd+C and paste clipboard text on Cmd+V, wrapped in bracketed-paste markers when the application enabled that mode. When that mode is off and the text contains a line break, the system SHALL ask before sending it, in a bar showing the number of lines: Enter pastes, Esc cancels. Programs SHALL be able to write the clipboard through OSC 52, but OSC 52 read requests SHALL be ignored without a reply.

#### Scenario: Paste into a bracketed-paste app
- **WHEN** Claude Code is running and the user presses Cmd+V
- **THEN** the pasted text is sent between `ESC[200~` and `ESC[201~`

#### Scenario: Multi-line paste without bracketed paste
- **WHEN** the pane runs `ssh` without bracketed paste and the user pastes 3 lines
- **THEN** nothing is sent and a bar asks "Pegar 3 líneas sin protección"; Enter sends the text, Esc discards it

#### Scenario: Single-line paste stays instant
- **WHEN** bracketed paste is off and the user pastes `git status` with no line break
- **THEN** the text is sent at once, with no bar

#### Scenario: Program reads the clipboard
- **WHEN** a program writes the OSC 52 query `ESC]52;c;?BEL`
- **THEN** no reply is written to the pane and the clipboard is not read

#### Scenario: Program writes the clipboard
- **WHEN** a program writes `ESC]52;c;aG9sYQ==BEL`
- **THEN** the clipboard holds `hola`
