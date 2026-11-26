# cardumen

Personal macOS terminal in Rust: clean, fast, private. No AI built in (Claude Code runs inside it), no network, no telemetry.

Stack: winit + wgpu (Metal) + `alacritty_terminal` + cosmic-text.

## Build and run

```sh
scripts/bundle.sh --install   # builds Cardumen.app and installs it in /Applications
```

Or, for development: `cargo run --release`.

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
| Cmd+C / Cmd+V | copy selection / paste (selecting text also copies it) |
| Cmd+= / Cmd+- / Cmd+0 | font size bigger / smaller / reset |
| Cmd+T / Cmd+W | new tab / close pane (closes the tab with its last pane) |
| Cmd+Shift+R, double-click a tab | rename the tab (Enter saves, Esc cancels, empty = automatic title) |
| Drag a tab, Cmd+Shift+←/→ | move the tab |
| `×` on a tab | close the tab and all its panes |
| Cmd+1…8, Cmd+9 | tab 1–8, last tab |
| Cmd+Shift+[ / ], Ctrl+Tab | previous / next tab |
| Cmd+D / Cmd+Shift+D | split right / split down |
| Cmd+Option+Arrows | focus the pane in that direction |
| Cmd+B | show / hide the session sidebar |
| Cmd+I | show / hide the Resumen panel (step, Claude, PR/MR, changes) |
| Cmd+U | usage dashboard (Esc closes) |
| Cmd+Q | quit |
| Option+←/→ | word left/right (`ESC b` / `ESC f`) |
| Cmd+←/→, Cmd+⌫ | line start/end, delete line |

## Usage dashboard

Cmd+U shows Claude Code usage read from `~/.claude/projects` (no network): tokens, tasks, agent time, tokens per day, a weekday × hour heatmap, and breakdowns by model and project, for 7 / 30 / 90 days or all time. Parsed data is cached in `~/Library/Caches/cardumen/usage.bin`; the first run reads everything (≈5 s for 3.6 GB), later runs only new bytes (≈30 ms).

## Measurements (Apple M1, 8 GB, one session)

- Memory: 39 MB `phys_footprint` (from `footprint`)
- Memory with 3 tabs and 5 shells: 44 MB
- Idle CPU: 0.0%

## Claude Code session states

The sidebar shows each Claude Code session as working, waiting for you, or done. It needs hooks in `~/.claude/settings.json` (installed once, with a backup; they do nothing outside Cardumen):

```sh
scripts/install-hooks.py
```

## Setting up on another Mac

```sh
brew install rustup gh            # glab instead of (or besides) gh for GitLab
rustup default stable
git clone <this repo> && cd cardumen
scripts/bundle.sh --install       # builds for that Mac's CPU and installs the app
scripts/install-hooks.py          # Claude Code hooks for the session sidebar
gh auth login                     # or: glab auth login
```

## Specs

Specs live in `openspec/`. Roadmap: `terminal-core` (this) → `tabs-and-splits` → `session-sidebar` (Claude Code status through hooks) → `usage-panel`.
