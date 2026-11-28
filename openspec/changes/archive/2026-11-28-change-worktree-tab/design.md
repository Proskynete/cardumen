## Context

- Active changes live in `openspec/changes/<name>/` (excluding `archive/`); `context.rs` already finds the OpenSpec root for a pane.
- Claude Code's own `--worktree` flag puts worktrees in `.claude/worktrees/<name>`, and this repo already ignores `/.claude/worktrees`.
- New tabs take a directory (`spawn_pane(.., cwd)`), and the session layer can wait until the shell is ready (bracketed paste on at the prompt).
- The Resumen panel follows the *shell's* directory, so the shell must start in the worktree, not just `claude`.

## Goals / Non-Goals

**Goals:** one choice in Cmd+P goes from a change name to Claude running in its own branch and tab; running it twice never creates a second worktree.

**Non-Goals:** removing worktrees after merge, creating changes (that stays `/opsx:propose`), choosing a base branch per change, GitLab/GitHub calls.

## Decisions

- **Cardumen runs `git worktree add`, not `claude --worktree`.** With `claude --worktree` the shell would stay in the main checkout and the panel would show the wrong branch.
- **Path and branch:** `<repo>/.claude/worktrees/<change>`, branch `<change>`. If the directory exists and is a worktree, reuse it. If the branch exists, check it out (`git worktree add <path> <change>`); otherwise create it from the main checkout's current `HEAD` (`git worktree add -b <change> <path> HEAD`). Using the local `HEAD` keeps committed proposals that are not pushed yet.
- **Ignore without touching tracked files:** if `git check-ignore` says `.claude/worktrees/` is not ignored, append `/.claude/worktrees/` to `.git/info/exclude` (local, never committed).
- **Start Claude by typing it.** Open the tab with the worktree as directory and the change as tab name; once the shell is ready, write `claude\r`. The user's shell config, aliases and PATH apply as if they typed it.
- **Uncommitted proposals are copied.** If `openspec/changes/<change>/` is missing in the new worktree, it is copied from the main checkout.
- **From inside a worktree, use the main checkout.** The repo is found by following the worktree's `.git` file to `<main>/.git`, without running git, so changes are never nested in another worktree.
- **Errors stay visible.** `git` runs off the UI thread with a timeout; on failure the switcher row shows the first line of git's error and no tab opens.

## Risks / Trade-offs

- [The change folder is not committed yet, so a worktree from `HEAD` lacks it] → it is copied into the worktree; the original stays.
- [The branch is already checked out in another worktree] → git refuses; its error is shown as is.
- [Worktrees pile up] → out of scope; `git worktree prune`/`remove` stays manual for now.

## Open Questions

None. Decided: an uncommitted change folder is copied into the new worktree (the original stays in the main checkout, so nothing is lost).
