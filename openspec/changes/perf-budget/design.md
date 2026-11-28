## Context

Session restore reads `~/Library/Application Support/cardumen/session.json`; with it, Cardumen opens tabs, splits and directories on launch. That is the cheapest way to start Cardumen in a known state without adding CLI flags. macOS `ps -o rss=,%cpu=` gives memory and CPU per process.

## Goals / Non-Goals

**Goals:** one command, same numbers every run on the same Mac, a clear pass/fail, never touching the user's real session.

**Non-Goals:** CI, benchmarks of rendering throughput, profiling, history of results, measuring Claude Code itself.

## Decisions

- **Override the session path by env, not a flag.** `CARDUMEN_SESSION` is two lines in `session_file::path()`; the script copies a fixture there, so the real session is never read or written.
- **Fixture:** 5 tabs, one with a split, all in temp directories outside any git repo, so no `git`/`gh` children run and results don't depend on the network.
- **First frame from inside the app.** With `CARDUMEN_STARTUP_LOG=1`, print `first frame: <ms>` once, measured from process start to the first presented frame. Timing from outside can't see when the window is drawn.
- **Memory and CPU from outside.** Wait 10 s for shells to settle, then sample `ps` once per second for 5 s: max RSS and mean CPU. Children (zsh) are not counted; the budget is Cardumen's own.
- **Budgets with headroom over the README:** 300 ms, 60 MB, 0.5%. The script prints each value next to its limit; limits are variables at the top so they can be tightened later.
- **Clean exit:** SIGTERM at the end; the temp session file and directories are deleted with a `trap`.

## Risks / Trade-offs

- [Numbers vary between Macs and with system load] → limits have headroom; run on the same machine to compare.
- [User shell startup (`.zshrc`) affects RSS of children, not Cardumen] → only Cardumen's pid is measured.
- [Window steals focus while it runs] → ~16 s; documented in the script header.
