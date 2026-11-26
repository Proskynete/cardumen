## ADDED Requirements

### Requirement: Compute the diff
For the focused pane's repository, the system SHALL compute the diff of the working tree against the same base as Cambios: the merge base of `HEAD` with `origin/HEAD`, `origin/main` or `origin/master` (first that exists), else `HEAD`. Untracked files that are not ignored SHALL be included as entirely added. The computation SHALL run off the UI thread when the viewer opens and when the user refreshes it, and the viewer SHALL show "Calculando diff…" until the first result.

#### Scenario: Uncommitted and untracked work
- **WHEN** a branch has one committed change, one modified file not yet committed and one new untracked file
- **THEN** the viewer lists all three files

### Requirement: Parse unified diffs
The system SHALL parse the unified diff into files, hunks and lines with old and new line numbers. It SHALL recognise new, deleted and renamed files (with the old path), binary files ("Binary files … differ") and "\ No newline at end of file" markers.

#### Scenario: Rename with edits
- **WHEN** `a.rs` was renamed to `b.rs` and one line changed
- **THEN** the file is shown as `b.rs`, renamed from `a.rs`, with one removed and one added line

#### Scenario: Binary file
- **WHEN** a PNG changed
- **THEN** the file is listed and its body says it is binary

### Requirement: Size limits
The system SHALL read at most 3 MB of diff in total and keep at most 5000 lines per file. When a limit is hit, the viewer SHALL show "diff truncado".

#### Scenario: Generated lockfile
- **WHEN** a lockfile changed by 40000 lines
- **THEN** its first 5000 lines are shown followed by "diff truncado"

### Requirement: Viewer layout
The viewer SHALL cover the pane area, between the sidebar and the Resumen panel and under the tab bar. On the left it SHALL list the files (about 240 pt wide) with path and +/− counts, highlighting the selected one. On the right it SHALL show the selected file's unified diff in the monospace font: hunk headers muted on a subtle band, added lines on a green tint, removed lines on a red tint, context lines plain, old and new line numbers in a muted gutter, and long lines cut with `…` without wrapping. Only visible lines SHALL be drawn.

#### Scenario: Reading a hunk
- **WHEN** a hunk replaces line 12 of a file
- **THEN** the removed line shows old number 12 on red and the added line shows new number 12 on green

### Requirement: Open, navigate and close
Clicking a file in Cambios SHALL open the viewer at that file; Cmd+G SHALL open it at the first file of the focused pane's repository. Esc or Cmd+G SHALL close it. While open, keystrokes other than Esc and Cmd shortcuts SHALL NOT reach any shell. Trackpad and wheel SHALL scroll the diff, or the file list when the pointer is over it; `j`/`k` and the arrow keys SHALL scroll by one line, Space/PageDown and Shift+Space/PageUp by one page, `n`/`p` SHALL select the next/previous file, clicking a file SHALL select it, and `r` SHALL recompute the diff.

#### Scenario: From the panel
- **WHEN** the user clicks `src/main.rs` in Cambios
- **THEN** the viewer opens with `src/main.rs` selected and its diff at the top

#### Scenario: Esc
- **WHEN** the viewer is open and the user presses Esc
- **THEN** the panes return with the same tab and pane focused

#### Scenario: Outside git
- **WHEN** the focused pane is not inside a git repository and the user presses Cmd+G
- **THEN** nothing opens
