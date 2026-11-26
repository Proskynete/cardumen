## Why

The interface works but reads as "a terminal drawing text": one monospace size, square boxes, text symbols for graphics. The owner compared three directions in the browser (mono polished, native macOS, hybrid) and chose **native macOS**: SF Pro for all interface text, rounded shapes, filled badges, a graphical step pipeline. The terminal text stays monospace.

## What Changes

- Renderer: anti-aliased rounded rectangles (and circles), and proportional text runs in several sizes and weights, shaped by cosmic-text.
- Tabs as rounded pills; panes as rounded cards with a header (title, branch, close) and an accent ring on the focused pane when a tab is split.
- Sidebar: tinted summary pills, uppercase group labels, rounded session rows with step badges and a branch/PR line.
- Resumen panel: title, Resumen / Cambios switch, pipeline drawn as a rail with circles, Claude and PR in cards, coloured change counts; the Cambios view lists every changed file.

## Capabilities

### New Capabilities
- `native-ui`: the visual language of Cardumen's interface.

### Modified Capabilities

## Impact

`renderer/` (shader SDF, text runs, atlas keys), `font.rs` (UI runs), `main.rs` (chrome drawing, pane headers, panel switch). No new dependencies. Memory impact: a few more atlas entries.
