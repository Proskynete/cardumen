## Why

Tabs can't be reordered, and closing one needs Cmd+W on each of its panes. Both are basic expectations from any tabbed terminal.

## What Changes

- Drag a tab sideways to move it; the others shift as it passes them.
- A `×` close button on the active tab and on the tab under the pointer; clicking it closes the tab and every pane in it.
- Cmd+Shift+Left / Cmd+Shift+Right move the active tab one place.

## Capabilities

### New Capabilities

### Modified Capabilities
- `tabs`: adds reordering and a close button.

## Impact

`layout::Tabs` gains `move_tab`; `main.rs` gains drag and hover state and a `Close` hit target.
