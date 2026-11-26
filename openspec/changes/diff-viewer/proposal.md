## Why

The Cambios tab only lists changed files with +/− counts. When the owner comes back to a Claude Code session they need to read what actually changed without leaving the terminal.

## What Changes

- A **diff viewer** over the pane area (between the sidebar and the Resumen panel, under the tabs): a file list on the left and the unified diff of the selected file on the right, with line numbers and tinted added/removed lines.
- Opened by clicking a file in Cambios (opens at that file) or with Cmd+G (first file); Esc or Cmd+G closes it. Keyboard and trackpad scrolling, next/previous file.
- The diff is computed off the UI thread against the same base as Cambios (merge base with the remote default branch, else `HEAD`), including untracked files, capped in size.

## Capabilities

### New Capabilities
- `diff-viewer`: computing, parsing and showing the full diff of a pane's repository.

### Modified Capabilities

## Impact

New `diff.rs` (git runner with a byte cap, unified-diff parser, untracked files, scroll math). `context.rs` exposes the diff base it already computes. New overlay and input handling in `main.rs`; one new `UserEvent`. No new dependencies.
