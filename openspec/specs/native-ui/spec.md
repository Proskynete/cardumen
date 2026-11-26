# native-ui Specification

## Purpose
TBD - created by archiving change native-ui. Update Purpose after archive.
## Requirements
### Requirement: Interface typography
All interface text (tabs, sidebar, panel, pane headers) SHALL use the system font (SF Pro) with a size hierarchy: panel title 16 pt bold, row titles 13 pt semibold, body 12–12.5 pt, metadata 11–11.5 pt, uppercase group/section labels 11 pt semibold. Terminal text and file paths SHALL stay in the configured monospace font. Text that does not fit SHALL end in `…`.

#### Scenario: Long tab title
- **WHEN** a tab title is wider than its pill
- **THEN** it is cut with `…` inside the pill

### Requirement: Shapes
The renderer SHALL draw anti-aliased rounded rectangles and circles. Tabs SHALL be 8-pt rounded pills (active: lighter fill). Panes SHALL be 12-pt rounded cards on a darker gutter, each with a 26-pt header showing its title, branch and a close button; when a tab has more than one pane, the focused pane SHALL have a 1.5-pt accent ring. Badges SHALL be 6-pt rounded pills tinted with their colour.

#### Scenario: Split tab
- **WHEN** a tab has two panes and the right one is focused
- **THEN** the right card has an accent ring and the left does not

#### Scenario: Close from header
- **WHEN** the user clicks × in a pane header
- **THEN** that pane closes as with Cmd+W

### Requirement: Sidebar rows
Session rows SHALL be 10-pt rounded, with a coloured status dot, the title and a step badge on the first line, the state and elapsed time on the second, and the branch and PR/MR on the third. The summary SHALL be two tinted pills ("N te espera", "N trabajando").

#### Scenario: Waiting session
- **WHEN** a session is waiting
- **THEN** its dot, state text and summary pill use the warm colour

### Requirement: Resumen panel
The panel SHALL have a title and a Resumen / Cambios switch. Resumen SHALL draw the step pipeline as a rail with circles (done: filled green; current: accent ring with halo; future: hollow), and show Claude's ask or last message and the PR/MR in rounded cards, plus a change summary with coloured counts. Cambios SHALL list every changed file (up to 30) with its counts.

#### Scenario: Switch to Cambios
- **WHEN** the user clicks "Cambios"
- **THEN** the panel lists the changed files instead of the summary

