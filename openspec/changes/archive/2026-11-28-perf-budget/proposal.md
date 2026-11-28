## Why

"Rápido y liviano" is Cardumen's main promise and the README states numbers (~45 MB with several sessions, 0% CPU idle), but nothing checks them. A runaway-memory bug already slipped in once (`c3bbca0`). Each new feature should prove it kept the budget.

## What Changes

- `scripts/budget.sh`: builds release, launches Cardumen with a fixed 5-tab session in a throwaway session file, measures time to first frame, physical footprint and idle CPU, prints them, and exits non-zero when one is over budget.
- Two environment variables, used only by the script: `CARDUMEN_SESSION` (session file path instead of the real one) and `CARDUMEN_STARTUP_LOG` (print time to first frame to stderr).
- Budgets: first frame ≤ 300 ms, memory (physical footprint) ≤ 60 MB, average idle CPU ≤ 0.5%.

## Capabilities

### New Capabilities
- `perf-budget`: a repeatable local check of startup time, memory and idle CPU against fixed limits.

### Modified Capabilities
- `session-restore`: the session file path can be overridden with `CARDUMEN_SESSION`.

## Impact

New `scripts/budget.sh`; `src/session_file.rs` (`path()` reads the override); `src/main.rs` (one stderr line on the first frame when asked). No new dependencies. It opens a real window, so it runs on the Mac, not in headless CI.
