## Context

`pull_request` runs `gh pr view --json number,title,state,isDraft,reviewDecision,url` or `glab mr view -F json` every 2 minutes per branch, off the UI thread. `open_prs` lists up to 100 open PRs with the same parser.

## Goals / Non-Goals

**Goals:** CI pass/fail/running on the branch PR card at zero extra process cost.

**Non-Goals:** CI on the open-PRs list (it would make `gh pr list` fetch checks for 100 PRs), per-check details, logs, re-running jobs, faster polling while CI runs.

## Decisions

- **Only the branch PR asks for checks.** `statusCheckRollup` is added to the `gh pr view` field list only; `gh pr list` keeps its fields. `Pr.ci` is `Option<Ci>` and stays `None` for list items. For GitLab the field already comes in `mr view`; list items ignore it.
- **One aggregate, worst wins:** any failed check (conclusion `FAILURE`, `TIMED_OUT`, `CANCELLED`, `ACTION_REQUIRED`, `STARTUP_FAILURE`, or status-context state `FAILURE`/`ERROR`) → failed; else any not finished (status not `COMPLETED`, or state `PENDING`/`EXPECTED`) → running; else passed (`SUCCESS`, `NEUTRAL`, `SKIPPED`); empty rollup → none. GitLab: `success` → passed; `failed`/`canceled` → failed; `running`/`pending`/`created`/`preparing`/`waiting_for_resource`/`scheduled` → running; missing or other (`skipped`, `manual`) → none.
- **Draw as a small pill after the state pill**, reusing `pill` with the success/warm/accent colors used elsewhere.

## Risks / Trade-offs

- [`statusCheckRollup` makes `gh pr view` slower on repos with many checks] → one PR, every 2 minutes, off the UI thread; measure on a real repo during verify.
- [CI state lags up to 2 minutes] → same as the PR state today; a turn ending already triggers a refresh.
- [`head_pipeline` absent in some glab versions] → missing field means "no CI", never an error.
