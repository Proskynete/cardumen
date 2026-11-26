## 1. Setup

- [ ] 1.1 Create the Cargo project `cardumen` with pinned dependencies (winit 0.30, wgpu, alacritty_terminal 0.26, cosmic-text, arboard, serde, toml, libc, pollster, bytemuck)
- [ ] 1.2 Add `.gitignore`, a README with build/run instructions, and `cargo build --release` working

## 2. Config and theme

- [ ] 2.1 `config.rs`: `Config` with serde defaults, load from `~/.config/cardumen/config.toml`, report parse errors to stderr (unit tests: missing file, partial file, invalid file)
- [ ] 2.2 `theme.rs`: arrecife palette, ANSI 16 colours, mapping from alacritty `Color`/`NamedColor`/indexed 256 to RGB (unit test: 256-palette cube and greys)

## 3. Session

- [ ] 3.1 `session.rs`: spawn `$SHELL -l` (fallback `/bin/zsh`) through `alacritty_terminal::tty` with TERM/COLORTERM in the env, `Term` behind `FairMutex`, `EventLoop` thread, listener that wakes winit through `EventLoopProxy`
- [ ] 3.2 Resize: window size to cols/rows, `Term::resize` + PTY `on_resize`
- [ ] 3.3 Close the window when the shell exits

## 4. Fonts and renderer

- [ ] 4.1 `font.rs`: FontSystem, configured family with Menlo fallback + warning, cell metrics at the scale factor, glyph rasterisation through SwashCache (mask and colour)
- [ ] 4.2 `renderer/atlas.rs`: shelf-packed glyph atlas with cache and clear-on-full
- [ ] 4.3 `renderer/shaders.wgsl` + `renderer/mod.rs`: wgpu surface, background quad pipeline, glyph pipeline, per-frame instance buffers from a grid snapshot
- [ ] 4.4 Attributes: bold/italic faces, dim, inverse, underline, strikethrough, wide characters
- [ ] 4.5 Cursor: block, beam and underline, blink, hollow when unfocused
- [ ] 4.6 Redraw only on change; handle scale-factor changes

## 5. Input

- [ ] 5.1 `input.rs`: KeyEvent to bytes (text, Ctrl codes, Option as Meta, named keys with modifiers, APP_CURSOR) with unit tests
- [ ] 5.2 Cmd shortcuts: Cmd+C copy, Cmd+V paste (bracketed when enabled), Cmd+Q quit, Cmd+Plus/Minus/0 font size
- [ ] 5.3 Mouse: drag, double-click and triple-click selection; mouse reporting to the app when its mode is on (Shift overrides)
- [ ] 5.4 Scroll: trackpad pixel deltas to lines; scrollback on the primary screen, arrows/mouse events on the alternate screen; snap to bottom on input

## 6. Welcome banner

- [ ] 6.1 `welcome.rs`: Tiburoncín grid to half-block truecolor ANSI (unit tests: pixel mapping and transparency)
- [ ] 6.2 Sysinfo through sysctl: chip, RAM, macOS version, uptime; banner layout with the narrow-window fallback
- [ ] 6.3 Feed the banner into `Term` before shell output; respect `welcome = false`

## 7. Verify

- [ ] 7.1 Manual check: zsh prompt with Nerd icons, `claude`, `vim`, `htop`, resize, selection/copy/paste, scrollback
- [ ] 7.2 Measure RSS with one session (target < 60 MB) and idle CPU; record the numbers in the README
