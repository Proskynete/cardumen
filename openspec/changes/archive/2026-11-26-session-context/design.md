## Context

The sidebar already knows each pane's Claude state through hooks. Everything else a returning user needs lives in files (git, OpenSpec, Claude transcripts) or behind `gh`/`glab`. The owner works on GitHub at home and GitLab at work, on two Macs.

## Decisions

### Worker thread, keyed by repository
One `context` thread wakes every second, and on demand (`Refresh(pane)` sent after a Stop or waiting event). It receives the current list of `(pane, cwd)` and groups panes by repo root. It recomputes git data for a repo at most every 5 s (or immediately when a refresh was requested), and PR/MR data at most every 120 s per branch. Results go back as `UserEvent::Context(pane, Context)`. The UI thread never runs a child process.

### Cheap sources first
- Branch: read `.git/HEAD` (follow `gitdir:` for worktrees). No process.
- OpenSpec: read `openspec/changes/*/tasks.md` and count `- [x]` / `- [ ]`. No process.
- Changes: `git diff --numstat <merge-base>` (working tree vs merge base, so uncommitted work counts) and `git rev-list --count @{u}..HEAD`. The base is `origin/HEAD`, falling back to `origin/main`, then `origin/master`.
- PR/MR: `gh pr view --json number,title,state,isDraft,reviewDecision` or `glab mr view -F json`, run in the repo directory, so they use the current branch.

### Hook payload
Hook lines become `<pane> <event> <json>`. The hook appends its stdin with newlines stripped (`tr -d '\n'`). `status.rs` parses the optional JSON with `serde_json`: `tool_name` and `tool_input` for permission requests, `transcript_path` for every event. On `stop`, the worker reads the last 256 KB of the transcript and takes the last assistant text block.

### Installer upgrade
`install-hooks.py` removes Cardumen entries (marker `CARDUMEN_SOCKET`) before adding the new ones, so upgrading is one command and other tools' hooks stay untouched.

### Layout
The Resumen panel is drawn as chrome, like the sidebar, and reserves 320 pt on the right. Pipeline chips are text with coloured dots. Claude's message is wrapped by character count, which is exact in a monospace grid.

## Risks / Trade-offs

- [The transcript format is internal to Claude Code] → The parser only needs `type == "assistant"` plus text content blocks. On failure the panel shows nothing; nothing crashes.
- [`gh`/`glab` are slow (~300 ms) or ask for authentication] → They only ever run in the worker, with a timeout. Errors show as text.
- [Repos without a remote default branch] → The changes section falls back to uncommitted changes only.
