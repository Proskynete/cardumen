## Why

The owner wants to see how much Claude Code work happens and where: tokens, sessions, time, models, projects, and when during the week. Claude Code already writes everything needed into its local transcripts.

## What Changes

- A full-window **Uso** view (Cmd+U, Esc to close) styled like the reference dashboard: four KPI cards with deltas and sparklines, tokens per day, a weekday × hour heatmap, and breakdowns by model and by project. Ranges: 7, 30 and 90 days, or all.
- A background reader over `~/.claude/projects/**/*.jsonl` (sessions and subagents) with an incremental on-disk cache, so only new bytes are parsed after the first run.

## Capabilities

### New Capabilities
- `usage-panel`: reading Claude Code usage locally and showing it.

### Modified Capabilities

## Impact

New `usage.rs` (parser, cache, aggregation, worker). New view in `main.rs`. Cache file at `~/Library/Caches/cardumen/usage.bin`. No network, no new dependencies.
