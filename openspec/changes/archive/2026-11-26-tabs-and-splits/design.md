## Context

`terminal-core` built a window with one `Session` (PTY + `Term`) drawn by one wgpu pipeline. `Session` is already independent of the window. This change puts many sessions into one window.

## Goals / Non-Goals

**Goals:** tabs, nested splits with even sizes, keyboard-first control, a new tab or pane opening in the current directory, staying light (each extra shell costs only its `Term` grid).

**Non-Goals:** dragging dividers to resize, reordering tabs by drag, detaching tabs into windows, multiple windows, session restore on relaunch. Those can come later.

## Decisions

### Layout tree per tab
`enum Node { Leaf(PaneId), Split { dir: Dir, children: Vec<Node> } }`. Splitting a leaf in the same direction as its parent adds a sibling, so three vertical splits give three even columns instead of 50/25/25. Otherwise the leaf becomes a new `Split` node. Closing a leaf removes it, and a split left with one child collapses into that child. `layout(rect) -> Vec<(PaneId, Rect)>` divides space evenly, minus 1-pt dividers. The tree is pure data, unit-tested without a window.
- Alternative: a flat list of panes with stored rects. Closing and nesting become ad-hoc.

### Pane ids in events
`Listener` carries a `PaneId`. `UserEvent::Term(PaneId, Event)` lets `Exit` close the right pane and `Title` update the right tab. `Wakeup` from any pane redraws the window.

### Working directory
`Session` keeps the shell's pid, which `alacritty_terminal::tty::Pty::child()` exposes before the PTY moves into the event loop. The cwd is read with `proc_pidinfo(pid, PROC_PIDVNODEPATHINFO)`. That is the shell's own cwd, which is correct whenever the user is at a prompt.

### Rendering
`Renderer::render` takes a list of `(Frame, origin)` and a chrome list (solid rects plus text runs for tab titles). The same quad pipeline draws everything. Text runs reuse the glyph atlas. The single pipeline and atlas need no new GPU resources.

### Tab bar in the titlebar
The content view is full-size and the titlebar is transparent, so the band at the top (34 pt) is part of the content. Tabs start at x = 78 pt (after the traffic lights). Each tab is up to 180 pt wide and shrinks evenly when space runs out. Clicks on a tab select it. Clicks on the empty band call `window.drag_window()`, so the window can still be moved.

### Shortcuts
Cmd+T, Cmd+W, Cmd+1…9, Cmd+Shift+[ / ], Cmd+D, Cmd+Shift+D, Cmd+Option+Arrows. They are intercepted before the key encoder and never reach the PTY.

## Risks / Trade-offs

- [Clicks in the titlebar band may be swallowed by AppKit for window dragging] → Keyboard shortcuts cover every action. Verify clicks manually.
- [Many panes × 10k-line scrollback raises memory] → Each `Term` grows only as lines are used. Measure with 5 tabs.
- [Reading the cwd of a busy shell (e.g. inside `claude`) returns the shell's cwd, not the program's] → Acceptable: Claude Code runs in the project root.
