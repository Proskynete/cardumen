## Context

A GUI app started by launchd gets a minimal environment (no `LANG`, sometimes no `SHELL`), and its working directory is `/`. Cardumen already starts shells in `$HOME` and runs them as login shells, so `.zprofile` sets `PATH`.

## Decisions

- **Plain shell script instead of `cargo-bundle`**: the bundle is one plist, one binary and one icon. A new tool would add a dependency for about 30 lines of shell.
- **Icon from code**: `scripts/icon.py` writes PNGs with the standard library only (zlib + struct), so it needs no Pillow. `iconutil` turns the iconset into `.icns`. The grid is read from `src/welcome.rs`, so the icon and the banner never drift apart.
- **Ad-hoc signature** (`codesign -s -`): Apple Silicon needs a valid signature to run. The app is built locally, so it has no quarantine flag and Gatekeeper does not block it.
- **Locale fallback** is `en_US.UTF-8`: it exists on every macOS install. It applies only when the app inherits no locale variable at all.

## Risks / Trade-offs

- [A different locale is wanted, e.g. Spanish] → Export `LANG` in `.zprofile`; it overrides the fallback.
