## Context

Claude Code runs as a normal program inside panes; Cardumen cannot see its state from the terminal stream. Claude Code hooks run shell commands, inheriting the environment of the `claude` process, which inherits the pane's shell environment.

## Decisions

- **Unix socket + `nc -U`** instead of status files: events arrive instantly with no polling, and `nc` ships with macOS. A listener thread forwards each line as `UserEvent::Status` through the winit proxy.
- **Pane identity through the environment** (`CARDUMEN_PANE`): needs no process-tree walking and works for nested shells.
- **`PermissionRequest` for "waiting"**: it fires exactly when Claude needs the user. `PostToolUse` moves the state back to working once a permission is granted and the tool ran.
- **Project = git root name**: walking up from the pane's cwd to the first `.git` matches how the owner thinks of projects (several panes in subfolders of one repo group together).
- **Sidebar drawn as chrome**, with the same text and rect primitives as the tab bar; no new rendering path.

## Risks / Trade-offs

- [A hook runs `sh` + `nc` on every tool call] → A few ms each, far below a tool's own cost.
- [Claude killed without `SessionEnd`] → The state stays until the pane closes or a new session reports; acceptable.
- [Editing a global settings file] → Backup first; only append entries; idempotent.
