## Why

Cardumen runs untrusted output all day (`cat`, `curl`, `npm install`, build logs) next to Claude Code sessions that handle tokens and keys. Two gaps let that output or a careless paste act on the user's behalf: any program can read the clipboard through OSC 52, and a multi-line paste into a shell without bracketed paste runs each line at once.

## What Changes

- OSC 52 clipboard **reads** are ignored: no reply is sent. Writes stay allowed.
- Pasting text with a line break while the app has not enabled bracketed paste asks first, in a bar like the find bar: "Pegar N líneas sin protección · Enter pega · Esc cancela".
- The status socket is created with mode `0600`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-session`: the Clipboard requirement adds the OSC 52 read block and the multi-line paste confirmation.
- `claude-status`: the Status socket requirement adds owner-only permissions.

## Impact

`src/main.rs` (`Event::ClipboardLoad`, `paste_text`, a confirmation bar next to the find bar), `src/status.rs` (`serve`). No new dependencies, no config.
