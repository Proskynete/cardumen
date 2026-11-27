## 0. Before starting

- [x] 0.1 Resolve the open question: copy the uncommitted change folder
- [x] 0.2 `quick-navigation` merged (#16); this change adds rows to its switcher

## 1. Changes and worktrees

- [x] 1.1 List active changes for a repo (`openspec/changes/*` minus `archive`), with a fixture test
- [x] 1.2 `ensure_worktree(repo, change) -> Result<PathBuf, String>`: reuse / check out / create from `HEAD`; `.git/info/exclude` when not ignored; test on a temp repo covering all three paths and the exclude line
- [x] 1.3 Run it off the UI thread with a timeout; report back through the existing update channel

## 2. Tab

- [x] 2.1 New tab with a given directory and name (reuse `new_tab`/`spawn_pane`), then write `claude\r` once the shell is ready
- [x] 2.2 Mark changes whose worktree is open in a tab (pane cwd under the worktree path)

## 3. Switcher

- [x] 3.1 "Nuevo · <change>" rows after the tabs, fuzzy-filtered; Enter starts the change
- [x] 3.2 Show git's error on the row when it fails

## 4. Verify

- [ ] 4.1 `cargo test`, `cargo clippy`; by hand: start a change, start it again (reuses), main checkout `git status` stays clean
