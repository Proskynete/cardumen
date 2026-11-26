## Context

`layout::Tabs` holds each tab's pane tree (`Node`), focused pane and custom name. Pane ids are runtime counters, and each pane's directory lives in its shell (`Session::cwd()`). Nothing survives a quit.

## Goals / Non-Goals

**Goals:** restore tabs, names, split trees, directories, active tab, focused panes, and sidebar/panel visibility; never fail to start because of the file.

**Non-Goals:** keeping shells alive (no daemon), restoring scrollback or running programs, window size and position, the usage dashboard, multiple windows.

## Decisions

### A separate serde model
`session_file::Saved { version, tabs, active, sidebar, panel }`, where each tab has `name`, `focused` (index among its leaves, left to right) and `root`. A node is `pane { cwd }` or `split { dir, children }`. Pane ids are not stored; restore assigns fresh ids from `State::next_id`. Storing the focused pane as a leaf index keeps runtime ids out of the file. `capture` and `rebuild` are pure and unit-tested.
- Alternative: derive serde on `Node`/`Tab`. That would write runtime pane ids to disk and still need a cwd per pane.

### Defensive rebuild
`rebuild` clamps `active` and each `focused` to valid ranges, drops splits with no children, collapses splits with one child, and drops tabs that end up empty. A saved directory that is not an existing directory becomes `$HOME`. A blank name becomes no name. A different `version`, invalid JSON or zero tabs makes `load` return `None`, and Cardumen opens one tab as before.

### Spawning restored panes
`spawn_pane` sizes a pane from the active tab's layout, so restore makes each tab active in turn while spawning its panes, then selects the saved active tab. The sidebar and panel flags are applied first, so grids get their final size. A pane whose shell fails to start is removed like a failed split.

### When to save
`about_to_wait` builds the snapshot without directories (no syscalls, a few small allocations) and compares it to the last one. When it differs, a save is scheduled 1 s later, and the event loop wakes for it. The save then reads each shell's directory. This catches every structural change (shortcuts, clicks, drags, renames, focus moves) without hooking each call site. `exiting` always saves, which also records the latest directories. When no tabs are left, the file is deleted.
- Alternative: mark dirty at each mutation site. Easy to miss one, and it touches more shared code.

### Atomic write
Write to `session.json.tmp` in the same directory, then `rename`. A crash mid-write leaves the previous file intact.

## Risks / Trade-offs

- [A `cd` alone does not trigger a save] → Directories are saved on quit and with every structural save. A crash loses only directory changes made since the last structural change.
- [A non-UTF-8 directory path is saved lossily] → It will not exist on restore, so that pane opens in `$HOME`.
- [Many panes start shells at once on launch] → Same cost as opening them by hand.
