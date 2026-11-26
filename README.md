# cardumen

Personal macOS terminal in Rust: clean, fast, private. No AI built in (Claude Code runs inside it), no network, no telemetry.

Stack: winit + wgpu (Metal) + `alacritty_terminal` + cosmic-text.

## Build and run

```sh
cargo build --release
./target/release/cardumen
```

Requires Rust (stable) and, for the default font, `FiraCode Nerd Font Mono` (it falls back to Menlo when the font is missing).

## Config

Optional, at `~/.config/cardumen/config.toml`. Every key has a default:

```toml
scrollback = 10000
welcome = true          # Tiburoncín + hardware info on start
# user_name = "Eduardo Álvarez"   # defaults to the macOS full name

[font]
family = "FiraCode Nerd Font Mono"
size = 14.0
```

## Shortcuts

| Keys | Action |
|------|--------|
| Cmd+C / Cmd+V | copy selection / paste |
| Cmd+= / Cmd+- / Cmd+0 | font size bigger / smaller / reset |
| Cmd+Q | quit |
| Option+←/→ | word left/right (`ESC b` / `ESC f`) |
| Cmd+←/→, Cmd+⌫ | line start/end, delete line |

## Measurements (Apple M1, 8 GB, one session)

- Memory: 39 MB `phys_footprint` (from `footprint`)
- Idle CPU: 0.0%

## Specs

Specs live in `openspec/`. Roadmap: `terminal-core` (this) → `tabs-and-splits` → `session-sidebar` (Claude Code status through hooks) → `usage-panel`.
