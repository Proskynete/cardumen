## 1. Data

- [x] 1.1 `Ci { Passed, Failed, Running }` and `Pr.ci: Option<Ci>`
- [x] 1.2 Add `statusCheckRollup` to the `gh pr view` fields only (not `gh pr list`)
- [x] 1.3 Parse the rollup in `gh_pr` and `head_pipeline.status` in `glab_mr` (worst wins); tests with fixtures for failed, running, passed, empty and missing

## 2. Card

- [x] 2.1 Draw a "✓ CI" / "✗ CI" / "● CI" pill after the state pill on the branch PR card

## 3. Verify

- [ ] 3.1 `cargo test`, `cargo clippy`; check by hand on a PR with running then passing checks; time `gh pr view` with and without `statusCheckRollup`
