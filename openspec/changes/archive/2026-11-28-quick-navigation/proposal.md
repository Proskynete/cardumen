## Why

With 4–5 Claude Code sessions open, the Dock bounce says *some* session needs you, but not which; finding it means scanning the sidebar. And past 8 tabs, Cmd+1…9 no longer reaches every tab, so moving between projects falls back to the mouse.

## What Changes

- **Cmd+J** jumps to the session waiting on you. Pressed again, it goes to the next waiting one, longest-waiting first. With none waiting, it does nothing.
- **Cmd+P** opens a quick switcher: a centered box with a text field and a list of tabs (title and project). Typing filters with fuzzy matching; ↑/↓ move, Enter opens, Esc closes.

## Capabilities

### New Capabilities
- `quick-switcher`: the Cmd+P box to find and open a tab by typing.

### Modified Capabilities
- `session-sidebar`: adds the Cmd+J jump to a waiting session.

## Impact

`src/main.rs` (two shortcuts, one overlay drawn like the find bar), a small new module for fuzzy scoring. Uses data the sidebar already has; no new threads, processes or dependencies. The README shortcut table gains two rows.
