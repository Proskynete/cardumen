## Context

Cardumen is a personal terminal for macOS. The machine it must run well on is an Apple M1 with 8 GB of RAM. The owner uses Claude Code for nearly all work, across 4–5 projects in parallel, so the terminal ships no AI features of its own. The look is inspired by Warp (clean, dark, good typography), not its features: there are no command blocks and no editor-style input bar. The shell is zsh, with an existing `.zshrc`. This change is the foundation. Tabs, splits, the Claude Code session sidebar and the usage panel come in later changes and must fit on top of it.

## Goals / Non-Goals

**Goals:**
- Daily-drivable single-window terminal: correct VT behaviour for Claude Code, vim and htop.
- Under 60 MB RSS, instant startup, near-0% CPU when idle.
- Crisp Nerd Font rendering on Retina displays.
- Tiburoncín welcome banner.
- A structure where one `Session` (PTY + `Term`) is independent of the window, so tabs and splits can hold many sessions later.

**Non-Goals:**
- Tabs, splits, the sidebar, the usage panel (later changes).
- Command blocks, input editor, shell integration marks.
- Ligatures, images (kitty/sixel), ligature shaping across cells.
- Linux and Windows.
- Any network access, auto-update or telemetry.

## Decisions

### Windowing and GPU: winit 0.30 + wgpu
The owner chose a hand-built stack (winit + wgpu) over GPUI, for full control and minimal memory. winit 0.30 is the latest stable release (0.31 is in beta). wgpu selects the Metal backend on macOS. The surface uses `PresentMode::Fifo` (vsync), and redraws are requested only on changes (`ControlFlow::Wait`).
- Alternative: raw Metal through `objc2-metal`. Faster to start up, but much more unsafe FFI code. wgpu is enough.

### VT emulation and PTY: `alacritty_terminal`
`alacritty_terminal::Term` holds the grid, scrollback, modes and selection. `alacritty_terminal::tty` spawns the PTY, and its `EventLoop` runs the I/O thread that parses output into `Arc<FairMutex<Term>>` and forwards events (title, bell, exit, clipboard requests, wakeup) through an `EventListener` that posts a winit `UserEvent` through an `EventLoopProxy`.
- Alternative: `vte` + custom grid + `portable-pty`. That would rebuild what Alacritty already gets right.

### Text: `cosmic-text` for font lookup, fallback and rasterisation
`cosmic-text`'s `FontSystem` finds the configured family through the system font database and handles fallback (Nerd icons, emoji, CJK). `SwashCache` rasterises glyphs to alpha masks (or colour bitmaps for emoji). The renderer does NOT use cosmic-text's layout for the grid. Each cell is shaped on its own, then placed at `col * cell_w`, `row * cell_h`. Cell size comes from the primary font's advance width for `M` and its ascent + descent + line gap, rounded to whole physical pixels.
- Alternative: `glyphon` (cosmic-text + wgpu). It is built for paragraphs, not a fixed grid, and adds a layer.
- Alternative: Core Text through FFI. Best macOS fidelity, but more unsafe code. Keep it in mind if text quality falls short.

### Renderer: two instanced pipelines and a glyph atlas
1. **Backgrounds**: one instance per cell with a non-default background (and selection, and the cursor block), drawn as a solid quad.
2. **Glyphs**: one instance per visible glyph: atlas UV rect, screen position, colour, and a flag for colour glyphs (emoji are drawn as RGBA, not tinted).

The atlas is one 2048×2048 RGBA8 texture with a simple shelf packer, cached by `(glyph id, font id, bold/italic)`. When it fills up, it is cleared and repopulated. Each frame copies a snapshot of the visible grid while holding the `Term` lock (the lock stays short), then builds instance buffers without the lock.

### Welcome banner: bytes through the normal parser
The banner is an ANSI string (truecolor SGR + `▀`) generated in Rust and fed into `Term` through the same `vte` parser before the PTY starts reading. It therefore lives in scrollback like any other output, and needs no special rendering. Hardware data comes from `sysctlbyname` (`machdep.cpu.brand_string`, `hw.memsize`, `kern.boottime`, `kern.osproductversion`) through `libc`, with no child processes.

### Config: serde + toml, all fields optional
`Config` derives `Deserialize` with `#[serde(default)]`. A missing file gives the defaults. A parse error gives the defaults, and the error is printed to stderr. There is no hot reload in this change.

### Input mapping
winit `KeyEvent`s map to bytes: `text` for printable input, a table for named keys (arrows, Home/End, PgUp/PgDn, F1–F12, with modifier variants in the `CSI 1;m` form), Ctrl+letter maps to C0 codes, and Option sends an ESC prefix (Option as Meta). The terminal intercepts Cmd shortcuts (Cmd+C, Cmd+V, Cmd+Q, Cmd+N for a new window later, Cmd+Plus/Minus for font size). They are never sent to the PTY. The `Term`'s `APP_CURSOR` mode switches arrows to the SS3 form.

### Module layout
```
src/main.rs        winit App (ApplicationHandler), wiring
src/config.rs      Config + load
src/session.rs     PTY + Term + EventLoop; resize; write
src/input.rs       KeyEvent -> bytes
src/renderer/      mod.rs (wgpu setup, frame), atlas.rs, shaders.wgsl
src/font.rs        FontSystem, cell metrics, rasterise
src/theme.rs       arrecife palette, ANSI 16
src/welcome.rs     banner string, tiburoncín grid, sysinfo
```

## Risks / Trade-offs

- [Text quality from swash can look slightly different from Core Text] → Use subpixel positioning off, gamma-correct blending in the shader, and compare against Terminal.app. Core Text remains the fallback plan.
- [The winit/wgpu APIs change between versions] → Pin exact minor versions in `Cargo.toml`.
- [alacritty_terminal's API is not stable (0.x)] → Pin `0.26`, and wrap it in `session.rs` so the rest of the code does not depend on it directly.
- [A memory target of < 60 MB] → One atlas texture (16 MB at 2048² RGBA; R8 for masks would be 4 MB, so masks and colour use separate textures if memory is tight). Measure with `footprint` / Activity Monitor before the change is closed.
- [The Term lock is held during rendering while heavy output arrives] → Snapshot the visible cells under the lock, then render without it.
