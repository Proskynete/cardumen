## Why

Warp's interface is pleasant, but it costs ~1 GB of RAM, ships telemetry, and bundles AI that is redundant when Claude Code does the work. Cardumen is a personal macOS terminal written in Rust: clean, fast, private, built for running 4–5 Claude Code sessions across projects in parallel. This first change delivers the daily-drivable core so everything after it (tabs, sidebar, usage panel) builds on a terminal that is already in use.

## What Changes

- New Rust binary `cardumen` (macOS, Apple Silicon first) that opens one window running the user's login shell (`zsh -l`), so `.zprofile`, `.zshrc`, prompt and plugins load unchanged.
- GPU-rendered text grid (winit + wgpu on Metal) with Nerd Font support, including icon glyphs and wide characters.
- Full VT emulation via the `alacritty_terminal` crate, so full-screen TUIs (Claude Code, vim, htop) render correctly.
- Keyboard input, Cmd+C / Cmd+V, mouse selection, trackpad scrollback, window resize.
- Welcome banner on each new session: Tiburoncín drawn in colour half-blocks plus a fastfetch-style hardware block (chip, RAM, macOS, uptime, shell, font).
- Default theme "arrecife" (the eduardoalvarez.dev palette), and an optional TOML config at `~/.config/cardumen/config.toml`.
- No network access and no telemetry of any kind.

Roadmap, as later changes: `tabs-and-splits`, `session-sidebar` (Claude Code status through hooks: working, waiting, done), `usage-panel` (charts read from `~/.claude/projects/**/*.jsonl`).

## Capabilities

### New Capabilities
- `terminal-session`: spawning the login shell in a PTY, VT emulation, input, clipboard, selection, scrollback, resize, and process exit.
- `text-rendering`: GPU grid rendering, font loading (Nerd Font), glyph fallback, colours, cursor, and theme.
- `welcome-banner`: the Tiburoncín plus hardware-info block printed when a session starts.
- `config`: the optional TOML configuration file and its defaults.

### Modified Capabilities

## Impact

- New Cargo project at the repository root.
- Dependencies: `winit`, `wgpu`, `alacritty_terminal`, `cosmic-text` (shaping, font fallback, rasterisation), `serde` + `toml`, `arboard` (clipboard), `libc` (sysctl).
- Performance target: under 60 MB RSS with one session, on an 8 GB M1.
