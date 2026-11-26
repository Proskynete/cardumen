## Why

Every relaunch starts from a single tab in `$HOME`. The owner keeps several projects open in named tabs with splits, and rebuilds that layout by hand each time.

## What Changes

- Cardumen saves the window's structure to `~/Library/Application Support/cardumen/session.json`: tab order, custom names, each tab's split tree, each pane's working directory, the active tab, the focused pane per tab, and whether the sidebar and the Resumen panel are visible.
- The file is written atomically, shortly after any structural change and on quit. Closing the last tab deletes it.
- On launch, a valid file rebuilds that structure. Each pane starts a fresh login shell in its saved directory (or `$HOME` when it is gone). Only the first pane shows the welcome banner.
- A missing, unreadable or invalid file falls back to today's single tab.

## Capabilities

### New Capabilities
- `session-restore`: saving and restoring the window structure across launches.

### Modified Capabilities

## Impact

New `session_file.rs` (serde model, conversion to and from `layout::Tabs`, atomic file I/O). `layout::Dir` derives serde. `main.rs` restores in `resumed`, checks for changes in `about_to_wait`, and saves in `exiting`. Shell processes and scrollback are not restored.
