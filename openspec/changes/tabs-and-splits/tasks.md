## 1. Model

- [ ] 1.1 `layout.rs`: `Node` tree with split (same-direction append), close (collapse single child), even `layout(rect)` with dividers, and directional neighbour lookup (unit tests)
- [ ] 1.2 `Session`: keep the shell pid; `cwd()` through `proc_pidinfo`; spawn in a given working directory (test: cwd matches)

## 2. Workspace

- [ ] 2.1 `UserEvent::Term(PaneId, Event)` and a per-pane `Listener`
- [ ] 2.2 `workspace.rs`: tabs, panes, active tab, focused pane per tab; new tab, split, close pane/tab, select tab; titles from OSC or cwd (unit tests on the pure parts)
- [ ] 2.3 Recompute pane rects on resize, layout change and font change; resize each PTY

## 3. Rendering

- [ ] 3.1 Renderer draws several frames at origins plus chrome rects and text runs
- [ ] 3.2 Tab bar: tabs, truncated titles, active marker, `+`; dividers between panes
- [ ] 3.3 Hollow, non-blinking cursor in unfocused panes

## 4. Input

- [ ] 4.1 Shortcuts: Cmd+T/W/1–9/Shift+[ ]/D/Shift+D/Option+Arrows
- [ ] 4.2 Mouse: click focuses a pane; selection, wheel and mouse reporting go to the pane under the pointer; tab clicks; drag the window from the empty band

## 5. Verify

- [ ] 5.1 Tests pass; screenshot with 3 tabs and a nested split
- [ ] 5.2 Memory with 5 tabs, recorded in the README
