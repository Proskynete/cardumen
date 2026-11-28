## Why

After Claude opens a PR, the next question is "did CI pass?", and today that means opening GitHub or GitLab. The PR card already polls the PR every 2 minutes; the CI result can ride along in the same call.

## What Changes

- The branch's PR/MR card shows the CI state next to its PR state: `✓ CI` (passed, success), `✗ CI` (failed, warm), `● CI` (running, accent), or nothing when there are no checks.
- GitHub: add `statusCheckRollup` to the existing `gh pr view --json` call. GitLab: read `head_pipeline.status` from the existing `glab mr view -F json` output.
- No new processes, no change to the 2-minute cadence. The newest-open-PRs list stays as it is.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-context`: the Pull or merge request requirement adds the CI state of the branch's PR/MR.

## Impact

`src/context.rs` (`Pr` gains a CI field, `gh_pr`/`glab_mr` parse it, the `gh pr view` field list), `src/main.rs` (draw it on the card). No new dependencies.
