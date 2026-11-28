# session-context Specification

## Purpose
TBD - created by archiving change session-context. Update Purpose after archive.
## Requirements
### Requirement: Step from OpenSpec and git
For a pane inside a git repository, the system SHALL compute the step in this pipeline: **Spec** (an OpenSpec change exists but `tasks.md` is missing), **Tareas** (tasks in progress, shown as `done/total`), **PR** (all tasks done or no OpenSpec change, and no PR/MR yet), **Review** (open PR/MR), **Merge** (merged). The active OpenSpec change SHALL be the most recently modified directory under `openspec/changes/` other than `archive`. When the repository has no OpenSpec, the pipeline SHALL start at PR.

#### Scenario: Tasks in progress
- **WHEN** `openspec/changes/session-context/tasks.md` has 7 of 12 boxes checked and the branch has no PR
- **THEN** the step is "Tareas 7/12" in change "session-context"

#### Scenario: Open PR
- **WHEN** every task is checked and the branch has an open PR #42
- **THEN** the step is "Review" and the PR shows as "#42 abierto"

### Requirement: Branch and changes
The system SHALL show the current branch (detached HEAD as the short commit) and the changes against the merge base with the remote default branch, including uncommitted work: number of files, lines added and removed, and up to 30 files with their own counts, plus the number of commits not pushed.

#### Scenario: Work in progress
- **WHEN** a branch changed 3 files (+120 −8) and has 2 unpushed commits
- **THEN** the panel shows "3 archivos · +120 −8" with the files, and "2 commits sin push"

### Requirement: Pull or merge request
The system SHALL find the PR/MR for the current branch with `gh` when the remote host contains `github`, and `glab` when it contains `gitlab`, showing number, title and state: draft, abierto, aprobado, cambios pedidos, mergeado or cerrado. The branch's PR/MR SHALL also show its CI state, read from the same `gh`/`glab` call: "✓ CI" when all checks passed, "✗ CI" when any failed, "● CI" while any is running, and nothing when it has no checks. When the CLI is missing or fails, the PR section SHALL say so and the rest SHALL keep working. PR/MR lookups SHALL run at most every 2 minutes per branch.

#### Scenario: GitLab at work
- **WHEN** the remote is `git@gitlab.company.com:team/app.git` and `glab` reports MR !17 opened
- **THEN** the panel shows "!17 abierto" with its title

#### Scenario: CLI not installed
- **WHEN** `glab` is not installed
- **THEN** the PR section shows "glab no está instalado"

#### Scenario: CI failed
- **WHEN** the branch PR #13 has one check with conclusion `FAILURE` and others `SUCCESS`
- **THEN** the card shows "#13 abierto" and "✗ CI"

#### Scenario: CI running
- **WHEN** the branch MR's `head_pipeline.status` is `running`
- **THEN** the card shows "● CI"

#### Scenario: No checks
- **WHEN** the branch PR has an empty `statusCheckRollup`
- **THEN** the card shows no CI state

#### Scenario: No extra process
- **WHEN** the PR is refreshed
- **THEN** exactly one `gh pr view` (or `glab mr view`) runs for the branch PR, as before

### Requirement: What Claude asked
When a pane is waiting, the panel SHALL show the tool and its main argument from the permission request (for example "Bash · git push origin main"). When a turn ends, the panel SHALL show the last text message Claude wrote in that session, wrapped to the panel width and limited to 12 lines.

#### Scenario: Permission request
- **WHEN** Claude asks permission to run `git push origin main`
- **THEN** the panel shows "pide permiso: Bash · git push origin main"

### Requirement: Resumen panel and sidebar line
A 320-point **Resumen** panel on the right SHALL show the focused pane's context, toggled with Cmd+I (visible by default). Sidebar rows SHALL show a third line with the step and the PR/MR number when there is one.

#### Scenario: Toggle
- **WHEN** the user presses Cmd+I
- **THEN** the panel hides and the panes take its space

### Requirement: Stay light
Context SHALL be computed off the UI thread. `git` runs SHALL be limited to one per repository every 5 seconds, plus one right after a Claude turn ends; panes in the same repository SHALL share results.

#### Scenario: Idle
- **WHEN** no pane is open in a git repository
- **THEN** no child processes are started

