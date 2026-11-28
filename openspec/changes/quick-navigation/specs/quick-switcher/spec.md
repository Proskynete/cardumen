## ADDED Requirements

### Requirement: Quick switcher
Cmd+P SHALL open a box over the panes with a text field and up to 8 tabs, each with its title, its project name and a warm dot when one of its panes is waiting. Typing SHALL filter and rank tabs by fuzzy match on title and project: the letters in order, ignoring case and accents, preferring consecutive letters and word starts. ↑/↓ SHALL move the selection, Enter SHALL open the selected tab and close the box, and Esc or a click outside SHALL close it. While it is open, keys SHALL not reach the pane.

#### Scenario: Open by typing
- **WHEN** tabs "cardumen", "blog" and "contabilidad" exist and the user types `cnt` then Enter
- **THEN** the "contabilidad" tab becomes active and the box closes

#### Scenario: Empty query
- **WHEN** the user presses Cmd+P and types nothing
- **THEN** the box lists tabs in tab-bar order with the active one selected

#### Scenario: No match
- **WHEN** the query matches no tab
- **THEN** the box shows "Sin resultados" and Enter does nothing

#### Scenario: Close
- **WHEN** the box is open and the user presses Esc
- **THEN** the box closes and the focused pane is unchanged
