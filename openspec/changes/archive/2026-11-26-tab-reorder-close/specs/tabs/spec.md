## ADDED Requirements

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
