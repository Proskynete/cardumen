## Why

The owner runs Claude Code in 4–5 projects at once and needs to see, at a glance, which session is waiting for them. Today that means clicking through tabs.

## What Changes

- Cardumen listens on a per-instance Unix socket and gives every shell `CARDUMEN_SOCKET` and `CARDUMEN_PANE`.
- Claude Code hooks (installed once in `~/.claude/settings.json`) report each session's state over that socket: working, waiting for the user, or done.
- A left sidebar (Cmd+B) lists every pane grouped by project, with its Claude Code state and how long it has been in it; clicking a row jumps to that pane.
- Tabs show a dot when a session in them is waiting; the Dock icon bounces when a session starts waiting while Cardumen is in the background.

## Capabilities

### New Capabilities
- `claude-status`: the socket protocol, the hooks and the state machine.
- `session-sidebar`: the sidebar UI, grouping, navigation and attention signals.

### Modified Capabilities

## Impact

New `status.rs` (socket listener and state machine) and `scripts/install-hooks.py`. Pane environment gains two variables. Layout reserves the sidebar width when it is visible. `~/.claude/settings.json` gains hook entries; existing hooks are kept.
