## Why

With 4–5 Claude Code sessions in parallel, coming back to one means re-reading it to answer: which step was it on, did Claude ask me something, is there an open PR/MR, what changed in the code. Cardumen should answer those in seconds, without AI and without losing its lightness.

## What Changes

- A background worker computes, per pane, a **session context** from local files and short CLI calls: git branch, OpenSpec progress, pull/merge request, and the code changed against the base branch.
- Hooks send their JSON payload, so Cardumen knows what Claude asked for permission to do and where the transcript is; when a turn ends, Cardumen reads Claude's last message from the transcript.
- A right **Resumen** panel (Cmd+I) shows, for the focused session: the step pipeline (Spec → Tareas → PR → Review → Merge), what Claude asked, the PR/MR, and the changed files.
- Sidebar rows gain a third line with the step and PR/MR.
- GitHub (`gh`) and GitLab (`glab`) are supported, picked from the remote URL.

## Capabilities

### New Capabilities
- `session-context`: deriving step, Claude's ask, PR/MR and changes for a session, and showing them.

### Modified Capabilities
- `claude-status`: hook lines carry the hook's JSON payload.

## Impact

New `context.rs` (pure parsers + worker thread). `status.rs` parses an optional JSON payload (adds `serde_json`). `install-hooks.py` upgrades existing Cardumen hook entries. New right panel in `main.rs`. Child processes: `git` every few seconds per repo with open panes, `gh`/`glab` every 2 minutes per branch.
