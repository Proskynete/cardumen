## ADDED Requirements

### Requirement: Changes in the quick switcher
The quick switcher SHALL list, after the tabs, each active OpenSpec change of the focused pane's repository (folders in `openspec/changes/` other than `archive`) as "Nuevo · <change>", filtered by the same fuzzy match. A change whose worktree is already open in a tab SHALL not get a "Nuevo" row; its tab is listed instead.

#### Scenario: Unstarted change
- **WHEN** the repo has changes `pr-ci-status` and `perf-budget` and no tab is in their worktrees
- **THEN** Cmd+P shows "Nuevo · pr-ci-status" and "Nuevo · perf-budget"

#### Scenario: Already open
- **WHEN** a tab's pane is in `.claude/worktrees/pr-ci-status`
- **THEN** Cmd+P shows that tab and no "Nuevo · pr-ci-status" row

### Requirement: Start a change in its worktree
Choosing "Nuevo · <change>" SHALL ensure a git worktree at `<repo>/.claude/worktrees/<change>` on branch `<change>`: reuse the worktree if it exists, check out the branch if it exists, else create the branch from the main checkout's `HEAD`. Then it SHALL open a tab in that directory named `<change>` and, once the shell is ready, run `claude`. If `.claude/worktrees/` is not ignored, the system SHALL add it to `.git/info/exclude`. If the change folder is not in the new worktree (not committed yet), the system SHALL copy it there from the main checkout, leaving the original. If git fails, no tab SHALL open and the row SHALL show git's error.

#### Scenario: First start
- **WHEN** the user picks "Nuevo · pr-ci-status" and neither the branch nor the worktree exists
- **THEN** branch `pr-ci-status` is created from `HEAD`, the worktree is added, and a tab named "pr-ci-status" opens there running `claude`

#### Scenario: Branch exists
- **WHEN** branch `pr-ci-status` exists but has no worktree
- **THEN** the worktree checks out that branch; no new branch is created

#### Scenario: Not ignored
- **WHEN** the repo does not ignore `.claude/worktrees/`
- **THEN** `/.claude/worktrees/` is appended to `.git/info/exclude` and `git status` in the main checkout does not list the worktree

#### Scenario: Proposal not committed
- **WHEN** `openspec/changes/pr-ci-status/` exists only as untracked files in the main checkout
- **THEN** the worktree gets a copy of that folder and the main checkout keeps its own

#### Scenario: Git fails
- **WHEN** the branch is already checked out in another worktree
- **THEN** no tab opens and the row shows git's error line
