## 1. Sources (pure, tested)

- [x] 1.1 `context.rs`: read branch from `.git/HEAD` (incl. worktrees)
- [x] 1.2 OpenSpec: active change and task progress; step pipeline from spec + PR state
- [x] 1.3 Parse `git diff --numstat`, provider from remote URL, `gh` and `glab` JSON
- [x] 1.4 Last assistant text from a transcript tail; permission summary from hook JSON

## 2. Plumbing

- [x] 2.1 Hook lines with JSON payload; `install-hooks.py` upgrades existing entries
- [x] 2.2 Worker thread with per-repo throttling and on-demand refresh; `UserEvent::Context`

## 3. UI

- [x] 3.1 Resumen panel (Cmd+I): pipeline, Claude's ask/last message, PR/MR, changes
- [x] 3.2 Sidebar third line: step and PR/MR

## 4. Verify

- [x] 4.1 Tests pass; screenshot of the panel on this repo
- [x] 4.2 README: setting up Cardumen on another Mac (Rust, gh/glab, bundle, hooks)
