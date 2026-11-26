## Context

`context::changes()` already picks the base (`merge-base HEAD origin/HEAD|origin/main|origin/master`, else `HEAD`) and runs `git diff --numstat`. Its `run()` helper waits for the child before reading stdout, which only works for small outputs.

## Decisions

- **Same base as Cambios**: `context::diff_base()` is extracted from `changes()` and reused, so the file list matches the panel.
- **Streaming read with a cap**: `git -c core.quotepath=false diff --no-color --no-ext-diff -M --src-prefix=a/ --dst-prefix=b/ <base>` is read directly from the pipe up to 3 MB; past that the child is killed and the result is marked truncated. Explicit prefixes make settings like `diff.noprefix` harmless.
- **Untracked files** come from `git ls-files --others --exclude-standard -z` (at most 200), read from disk within the remaining byte budget and shown as all-added; a NUL byte in the first 8 KB marks them binary. The Cambios counts (numstat) still exclude them.
- **Pure parser**: `parse(&str) -> Vec<File>` walks the output and uses the hunk header counts to tell content lines from headers (so a removed line starting with `--` is not a header). Each file keeps its status (modified, new, deleted, renamed with the old path), binary flag, counts and lines (hunk, added, removed, context, note) with old/new numbers. Lines past 5000 in a file are dropped and the file is marked truncated.
- **Short-lived worker**: opening, refreshing (`r`) or reopening spawns a thread that runs `diff::load` and sends `UserEvent::Diff(repo, Diff)`, like the usage view.
- **Row-snapped virtualised drawing**: the renderer has no clipping, so scroll offsets are kept in pixels (smooth trackpad accumulation) but rows are drawn from a whole-row index; only visible rows become chrome. `visible()`, `clamp()` and `reveal()` are pure and tested.
- **Text**: diff text uses `Face::Mono`; tabs become four spaces; each visible line is pre-cut by character count before the label's own `…` cut, so long lines cost little.

## Risks / Trade-offs

- [Huge diffs] → 3 MB / 5000-line caps with "diff truncado"; only visible rows are drawn.
- [git hangs] → the read has no timeout; `git diff` does not wait for input (stdin is closed) or locks.
- [Paths containing ` b/`] → `+++` and `rename to` lines override the `diff --git` header path; only binary files or pure renames with such names could show a wrong path.
