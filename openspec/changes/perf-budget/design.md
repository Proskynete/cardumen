## Context

Session restore reads `~/Library/Application Support/cardumen/session.json`; with it, Cardumen opens tabs, splits and directories on launch. That is the cheapest way to start Cardumen in a known state without adding CLI flags. `vmmap --summary` gives a process's physical footprint (what Activity Monitor shows) and `ps -o %cpu=` its CPU.

## Goals / Non-Goals

**Goals:** one command, same numbers every run on the same Mac, a clear pass/fail, never touching the user's real session.

**Non-Goals:** CI, benchmarks of rendering throughput, profiling, history of results, measuring Claude Code itself.

## Decisions

- **Override the session path by env, not a flag.** `CARDUMEN_SESSION` is two lines in `session_file::path()`; the script copies a fixture there, so the real session is never read or written.
- **Fixture:** 5 tabs, one with a split, all in temp directories outside any git repo, so no `git`/`gh` children run and results don't depend on the network.
- **First frame from inside the app.** With `CARDUMEN_STARTUP_LOG=1`, print `first frame: <ms>` once, measured from process start to the first presented frame. Timing from outside can't see when the window is drawn.
- **Memory and CPU from outside.** Wait 10 s for shells to settle, then sample once per second for 5 s: max physical footprint and mean CPU. RSS was tried first and swung between 12 and 103 MB for the same session (shared and GPU-mapped pages, memory pressure); the footprint stays within ~1 MB. Children (zsh) are not counted; the budget is Cardumen's own.
- **Budgets with headroom over the README:** 300 ms, 60 MB, 0.5%. The script prints each value next to its limit; limits are variables at the top so they can be tightened later.
- **Warm-up launch, not measured.** The first launch of a freshly built binary took 0.6–2 s (macOS checks the new binary; fonts load cold), later ones ~200 ms. Daily launches are the warm case, so the script launches once, kills it, and measures the second launch.
- **Clean exit:** SIGTERM at the end; the temp session file and directories are deleted with a `trap`.

## Risks / Trade-offs

- [Numbers vary between Macs and with system load] → limits have headroom; run on the same machine to compare.
- [User shell startup (`.zshrc`) affects the memory of children, not Cardumen] → only Cardumen's pid is measured.
- [Window steals focus while it runs] → ~16 s; documented in the script header.
