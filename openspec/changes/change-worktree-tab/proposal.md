## Why

Every change follows the same path: propose it in OpenSpec, then work on it in its own branch, often in parallel with other changes. Today that start is manual: create the worktree and branch, open a tab, `cd`, rename the tab, run `claude`. It is the same five steps every time, and the one place where parallel work trips over itself.

## What Changes

- The Cmd+P quick switcher (from `quick-navigation`) also lists the active OpenSpec changes of the focused pane's repository that have no tab yet, as "Nuevo · <change>".
- Choosing one creates (or reuses) a git worktree at `<repo>/.claude/worktrees/<change>` on branch `<change>`, opens a tab there named after the change, and starts `claude` in it.
- If a tab already has that worktree open, the switcher shows that tab instead of a "Nuevo" row.

## Capabilities

### New Capabilities
- `change-worktree`: starting work on an OpenSpec change in its own worktree and tab.

### Modified Capabilities

## Impact

Depends on `quick-navigation` (the switcher). `src/main.rs` (new tab with a given directory, name and first command), `src/context.rs` or a small module for listing changes and running `git worktree add`. Writes to `.git/info/exclude` when `.claude/worktrees/` is not ignored. No new dependencies.
