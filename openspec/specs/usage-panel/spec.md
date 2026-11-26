# usage-panel Specification

## Purpose
TBD - created by archiving change usage-panel. Update Purpose after archive.
## Requirements
### Requirement: Read usage locally
The system SHALL read every `*.jsonl` under `~/.claude/projects` (including `subagents/`). Each assistant message with `message.usage` SHALL count once per `message.id`. Per message it SHALL keep: time, session, project (git root name of `cwd`, else its folder name), model, input, cache-read, cache-write and output tokens. A user turn SHALL be a `user` line whose content is the user's text (not a tool result, command output or meta line).

#### Scenario: Streaming duplicates
- **WHEN** a transcript has three lines for the same `message.id` with the same usage
- **THEN** that message counts once

### Requirement: Incremental cache
Parsed records SHALL be stored in `~/Library/Caches/cardumen/usage.bin` with each file's size and read offset. On later loads, only bytes after the stored offset SHALL be parsed. A file that shrank SHALL be re-read from the start. Reading SHALL happen off the UI thread, and the view SHALL show "Leyendo transcripts…" until the first result.

#### Scenario: Second open
- **WHEN** the view is opened again after one new message was written
- **THEN** only the new bytes are parsed and the numbers include that message

### Requirement: Metrics
For the selected range (7, 30, 90 days or all, ending today), the view SHALL show:
- **Tokens de entrada**: input + cache read + cache write, with the cache-read share.
- **Tokens de salida**: output, with output per turn.
- **Tareas**: number of sessions with activity, with the number of turns.
- **Tiempo de agente**: the sum, per session, of gaps of at most 5 minutes between consecutive messages, with the highest number of sessions active in the same 5-minute window.
Each card SHALL show the change against the previous period of the same length (green when higher, red when lower, hidden for "all") and a per-day sparkline.

#### Scenario: Agent time
- **WHEN** a session has messages at 10:00, 10:03 and 10:30
- **THEN** it adds 3 minutes of agent time

### Requirement: Charts and breakdowns
The view SHALL show tokens per day (input bars up, output bars down, with the maximum of each labelled), a weekday × hour heatmap of messages (local time, Monday first, with the busiest slot named), and two tables: **Modelos** (input, output, tasks, with a share bar) and **Proyectos** (input, output, agent time, with a share bar), each sorted by input tokens and limited to the top 6 plus an "N más" row.

#### Scenario: Range switch
- **WHEN** the user clicks "7 días"
- **THEN** every card, chart and table recomputes for the last 7 days

### Requirement: Open and close
Cmd+U and a "Uso ⌘U" button at the bottom of the sidebar SHALL open the view; Cmd+U, Esc and the "esc" button SHALL close it. While it is open, the tabs and panes are hidden and keystrokes other than Esc and Cmd shortcuts SHALL NOT reach any shell.

#### Scenario: Esc
- **WHEN** the view is open and the user presses Esc
- **THEN** the terminal returns, with the same tab and pane focused

