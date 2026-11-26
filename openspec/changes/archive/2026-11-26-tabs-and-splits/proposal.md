## Why

The owner runs Claude Code in 4–5 projects in parallel. A single shell per window means juggling many windows. Tabs (one per project) and split panes (Claude Code next to a plain shell in the same project) are the base for the session sidebar that comes next.

## What Changes

- Tabs in the transparent titlebar band, next to the traffic lights: title, active marker, `+` button.
- Split panes inside a tab, right or down, nested freely, with even sizes.
- Keyboard shortcuts for creating, closing, switching and focusing.
- A new tab or pane starts in the working directory of the focused pane.
- Each pane is an independent `Session`; the renderer draws many sessions per frame.
- The welcome banner shows on new tabs, not on new split panes.

## Capabilities

### New Capabilities
- `tabs`: creating, closing, selecting and titling tabs, and the tab bar.
- `split-panes`: splitting, closing, focusing and laying out panes inside a tab.

### Modified Capabilities

## Impact

- `main.rs` moves from one session to a workspace of tabs → pane trees → sessions.
- `UserEvent` carries the pane id so terminal events reach the right pane.
- Renderer draws several frames at offsets, plus chrome (tab bar, dividers).
- `session.rs` exposes the shell pid, so its cwd can be read with `proc_pidinfo`.
