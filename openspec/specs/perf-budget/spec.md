# perf-budget Specification

## Purpose
TBD - created by archiving change perf-budget. Update Purpose after archive.
## Requirements
### Requirement: Budget script
`scripts/budget.sh` SHALL build the release binary, launch it once unmeasured (a freshly built binary's first launch pays for macOS checking it and loading fonts cold), then launch it again with a fixed 5-tab session outside any git repository, and report time to first frame, peak physical footprint (the memory Activity Monitor shows) and mean idle CPU of the Cardumen process. It SHALL exit non-zero when first frame exceeds 300 ms, memory exceeds 60 MB, or idle CPU exceeds 0.5%, and SHALL print each value next to its limit. It SHALL not read or change the user's real session file, and SHALL remove its temporary files on exit.

#### Scenario: Within budget
- **WHEN** the user runs `scripts/budget.sh` and Cardumen uses 45 MB, 0.1% CPU and draws in 180 ms
- **THEN** the script prints the three values with their limits and exits 0

#### Scenario: Over budget
- **WHEN** the physical footprint peaks at 75 MB
- **THEN** the script marks memory as over its 60 MB limit and exits non-zero

#### Scenario: Real session untouched
- **WHEN** the script finishes
- **THEN** `~/Library/Application Support/cardumen/session.json` never contains the fixture tabs, and no Cardumen process started by the script is left running

### Requirement: Startup log
When `CARDUMEN_STARTUP_LOG` is set, Cardumen SHALL print `first frame: <ms>` to stderr once, with the time from process start to the first presented frame. Without it, nothing SHALL be printed.

#### Scenario: Log on
- **WHEN** Cardumen starts with `CARDUMEN_STARTUP_LOG=1`
- **THEN** stderr gets one line like `first frame: 182`

