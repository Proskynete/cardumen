## MODIFIED Requirements

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
