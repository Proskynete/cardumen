## Why

Cardumen is a bare binary launched from another terminal. To be the daily terminal it must be a real macOS app: in /Applications, the Dock and Spotlight, with its own icon.

## What Changes

- `scripts/bundle.sh` builds `Cardumen.app` (release binary, `Info.plist`, `.icns` icon), signs it ad hoc, and with `--install` copies it to `/Applications`.
- App icon generated from the Tiburoncín pixel grid on the arrecife background.
- When launched from Finder or the Dock, the shell still gets a usable environment: `SHELL` falls back to the user's login shell from the user database, and `LANG` defaults to `en_US.UTF-8` when no locale variable is set.

## Capabilities

### New Capabilities
- `app-bundle`: building, installing and launching Cardumen as a macOS app.

### Modified Capabilities

## Impact

New `scripts/bundle.sh` and `scripts/icon.py`. `session.rs` changes how the shell and the locale are chosen. No new Rust dependencies.
