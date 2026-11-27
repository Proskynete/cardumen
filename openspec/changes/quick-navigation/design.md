## Context

The sidebar already knows each pane's Claude state and since when (`status::Claude::Waiting`, `since`). Jumping to a pane already exists for sidebar clicks ("Jump to a session"). The find bar (Cmd+F) is the existing pattern for a text field over the panes with its own key handling.

## Goals / Non-Goals

**Goals:** reach the waiting session in one key; reach any tab in a few keystrokes; no new state beyond the open box.

**Non-Goals:** a command palette (actions, settings), searching scrollback or files, ranking by history or frequency.

## Decisions

- **Cmd+J order: longest-waiting first, then cycle.** Take waiting panes sorted by `since`; jump to the first one after the focused pane in that order, wrapping. Repeated presses visit each one without extra state.
- **Reuse the sidebar-click jump.** Cmd+J and Enter in the switcher call the same "activate tab and focus pane" path.
- **Switcher items are tabs, not panes.** The label is the tab title plus the project name the sidebar computes; a tab with a waiting pane shows the warm dot. Panes inside splits are one Cmd+Option+arrow away.
- **Fuzzy match: in-order subsequence, case- and accent-insensitive**, scored by consecutive runs and matches at word starts; ties keep tab order. ~40 lines with tests instead of a crate.
- **Overlay like the find bar.** Same input routing (keys go to the box while open), closes on Esc, on Enter, and on a click outside. Shows at most 8 rows.

## Risks / Trade-offs

- [Cmd+J and Cmd+P are taken by some shells' habits] → Cmd+letter never reaches the shell in Cardumen today, so nothing is lost.
- [Fuzzy scoring disagrees with intuition] → tests pin the common cases (prefix beats scattered match).
